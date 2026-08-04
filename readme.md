# Aillen

Opinionated, feature-incomplete audio engine, DSP library, and live synthesizers.

## Project Structure

This project is set up as a Cargo Workspace containing:

- `aillen-core`: A modular DSP library hosting mathematical primitives, oscillators, filters (including Biquad, DJ performance, Formant, and Comb filters), ADSR envelopes, sidechainable dynamic effects (Compressor, AM/Ring Modulator), wavefolding saturators, bitcrushing degraders, a stereo delay (Tape and Granular modes), a sequential track `FxChain`, and instrument implementations (including a 2-operator FM synth, a sampler, a 303 bass synth, a rave hubass synth, and a sample bank).
- `aillen-cli`: A standalone performance synthesizer that wraps `aillen-core` with real-time stereo audio (`cpal`) and an asynchronous UDP OSC server mapped via lock-free channels (`crossbeam-channel`).

---

## Signal Flow Graph

The following ASCII diagram illustrates the audio signal path from the instruments to the final stereo hardware output, highlighting the track-level `FxChain` inserts, the send/return routing, and the master bus:

```
                            +---------------------------------------+
                            |              TRACK                    |
                            |  +---------------+                 L  |
                            |  |  Instrument   |---------------+--->|
                            |  +---------------+               | R  |
                            |                                  v    |
                            |                            +---------+|
                            |                            | FxChain ||
                            |                            +---------+|
                            |                            /    |     |
                            |               (Send level)      v     |
                            |                    |       +---------+|
                            |                    |       | Panner  ||
                            |                    |       +---------+|
                            |                    |            |     |
                            +--------------------|------------|-----+
                                                 |            |
                                                 v            v
                                            [Sum Sends]  [Sum Dry]
                                                 |            |
                                                 v            |
                                         +---------------+    |
                                         | Return Delay  |    |
                                         | (100% Wet)    |    |
                                         +---------------+    |
                                                 |            |
                                                 v            v
                                                 +----> [Mixer Summing]
                                                             |
                                                             v
                                                    +-----------------+
                                                    |  Master Volume  |
                                                    +-----------------+
                                                             |
                                                             v
                                                    +-----------------+
                                                    | Master DJFilter |
                                                    +-----------------+
                                                             |
                                                             v
                                                    +-----------------+
                                                    | Master WaveLoss |
                                                    +-----------------+
                                                             |
                                                             v
                                                    Stereo Output (DAC)
```

### Track FxChain Detail

Each track owns an independent `FxChain` containing a sequential arrangement of stereo processors:

```
                                     FxChain (Stereo)
                                     
                                   L Input     R Input
                                      |           |
                                      v           v
                                 +---------+ +---------+
                                 |AmRingMod| |AmRingMod| <--- Sidechain (optional)
                                 +---------+ +---------+
                                      |           |
                                      v           v
                                 +---------+ +---------+
                                 |WaveFoldr| |WaveFoldr|
                                 +---------+ +---------+
                                      |           |
                                      v           v
                                 +---------+ +---------+
                                 |Distortn | |Distortn |
                                 +---------+ +---------+
                                      |           |
                                      v           v
                                 +---------+ +---------+
                                 |Bitcrshr | |Bitcrshr |
                                 +---------+ +---------+
                                      |           |
                                      v           v
                                 +---------+ +---------+
                                 |CombFiltr| |CombFiltr|
                                 +---------+ +---------+
                                      |           |
                                      v           v
                                 +---------+ +---------+
                                 |DjFilter | |DjFilter |
                                 +---------+ +---------+
                                      |           |
                                      v           v
                                 +---------+ +---------+
                                 |Compressr| |Compressr| <--- Sidechain (optional)
                                 +---------+ +---------+
                                      |           |
                                      v           v
                                   L Output    R Output
```

---

## 1. CLI Usage

### Compiling

To build the entire workspace optimally, use the `--release` flag:

```bash
cargo build --workspace --release
```

### Starting the Synth

```bash
# Start with default settings (8 voices, port 8000, and scanning ~/Desktop/KairosSamples)
cargo run -p aillen-cli --release

# Start with a custom sample directory
cargo run -p aillen-cli --release -- --samples-dir "/path/to/my/breaks"

# Start with specific options
cargo run -p aillen-cli --release -- --port 9000 --voices 4
```

### Command-Line Arguments

The standalone executable accepts the following arguments:

- `-p`, `--port <PORT>`: UDP port to listen for incoming OSC packets (default: `8000`).
- `-v`, `--voices <VOICES>`: Number of polyphonic synth voices to allocate (default: `8`).
- `-d`, `--device-index <DEVICE_INDEX>`: Optional index of the host audio output device to use.
- `-l`, `--list-devices`: Flags to list all available host output devices and exit.
- `-s`, `--samples-dir <SAMPLES_DIR>`: Path to a directory containing audio samples to load on startup.

### Audio Device Management

If you have multiple audio interfaces, use these flags to select the correct one:

```bash
# List all available output devices and their indices
cargo run -p aillen-cli -- --list-devices

# Start using a specific device by index (e.g., index 2)
cargo run -p aillen-cli -- --device-index 2
```

### Terminal User Interface (TUI)

The CLI launches an interactive ASCII Terminal User Interface (TUI) designed for standard terminals (configured for exactly `80 columns x 43 rows`).

- **Layout Grid**:
  - **Top (Rows 0-30)**: Displays active track waveforms (up to 4 track slots in a 2x2 grid) with vertical left/right channel oscilloscope scopes and real-time track OSC command logs.
  - **Bottom-Left (Rows 31-42)**: Displays the master output volume horizontal oscilloscope scope and master-level OSC logs.
  - **Bottom-Right (Rows 31-42)**: Plots a real-time **L x R XY Vectorscope** displaying phase correlation, stereo field width, and alignment.
- **LRU Dynamic Display**: Out of the 8 available tracks, only the 4 most recently active tracks (based on audio activity or incoming OSC messages) are displayed. When a track goes inactive for 15 seconds, it is swapped out for a more recently active track.
- **Sticky Slots**: Tracks remember their last preferred slot index (`preferred_slot`) to prevent disorienting jumps when tracks are dynamic swapped.

---


## 2. Audio Mixer & Instrument Tracks

The engine supports a stereo Mixer with 8 instrument tracks and one delay return track:

- **Track 0**: `TwoOp` (FM Synth)
- **Track 1**: `Sampler` (Sample playback engine with multi-format support via Symphonia)
- **Track 2**: `Sampler`
- **Track 3**: `Sampler`
- **Track 4**: `TwoOp` (FM Synth)
- **Track 5**: `Sampler`
- **Track 6**: `Synth303` (Roland 303-like monophonic/legato bass synth)
- **Track 7**: `SynthHubass` (Versatile Rave & Bass Synthesizer with detuned unison, filter-bypassed sub-bass, multi-mode filters, drive, LFO, and stereo chorus)
- **Return Track**: A stereo delay effect track (100% wet by default).

All OSC messages must target the appropriate track path (`/track/<id>/`) or mixer path (`/mixer/`).

### Mixer & General Track Controls

| Address | Arguments | Argument Types | Default Value | Reasonable Range / Description |
| :--- | :--- | :--- | :--- | :--- |
| `/panic` / `/mixer/panic` | None | - | - | Panic command. Immediately silences all active notes on all tracks. |
| `/mixer/master/volume` | `f` | `f32` | `1.0` | Master output volume gain factor (`0.0` to `2.0+`). |
| `/mixer/master/filter` | `f` | `f32` | `0.0` | Master output DJ filter position. Low-Pass sweep: `-1.0` to `0.0` (20000 Hz down to 20 Hz). High-Pass sweep: `0.0` to `1.0` (20 Hz up to 20000 Hz). Center `0.0` is bypass. |
| `/mixer/master/waveloss/drop` | `i` | `i32` / `usize` | `0` | Master WaveLoss drop segments count. Range: `0` (bypass) to `outof` value. |
| `/mixer/master/waveloss/outof` | `i` | `i32` / `usize` | `40` | Master WaveLoss total segments per cycle. Range: `1` to `1000`. |
| `/mixer/master/waveloss/mode` | `i` | `i32` / `usize` | `1` | Master WaveLoss mode. `1` = deterministic, `2` = random. |
| `/mixer/master/limiter/gain` | `f` | `f32` | `1.0` | Pre-limiter gain boost multiplier. Range: `1.0` to `10.0+`. |
| `/mixer/master/limiter/release` | `f` | `f32` | `0.05` | Limiter release time in seconds. Range: `0.01` to `1.0`. |
| `/mixer/master/limiter/ceiling` | `f` | `f32` | `0.99` | Brickwall output ceiling amplitude limit. Range: `0.01` to `1.0` (e.g. `0.99` for -0.1 dBFS). |
| `/track/<id>/volume` | `f` | `f32` | `0.8` | Individual track volume gain factor. Range: `0.0` to `2.0+`. |
| `/track/<id>/pan` | `f` | `f32` | `0.0` | Track panning position. Range: `-1.0` (Hard Left) to `1.0` (Hard Right). |
| `/track/<id>/mute` | `i`/`b` | `i32` / `bool` | `0` (false) | Mute (`1` / `true`) or unmute (`0` / `false`) the track. |
| `/track/<id>/send/delay` | `f` | `f32` | `0.0` | Send level to the delay return track. Range: `0.0` (dry) to `1.0` (maximum send). |
| `/track/<id>/sidechain/source` | `i` | `i32` | `-1` | Set sidechain source track index. Range: `0` to `7`. Negative value (e.g., `-1`) disables it. |

### Mixer Return Delay Controls

These parameters control the stereo delay return track:

| Address | Arguments | Argument Types | Default Value | Reasonable Range / Description |
| :--- | :--- | :--- | :--- | :--- |
| `/mixer/return/delay/time` | `f` | `f32` | `0.3` | Delay time in seconds. Range: `0.01` to `3.0` (Tape) or `4.0` (Granular). |
| `/mixer/return/delay/feedback` | `f` | `f32` | `0.5` (Tape) / `0.4` (Gran) | Delay feedback volume factor. Range: `0.0` to `1.0` (avoid values `>= 1.0` to prevent feedback loops). |
| `/mixer/return/delay/mode` | `i` | `i32` / `usize` | `0` | Delay engine mode. `0` = Tape Delay, `1` = Granular Delay. |
| `/mixer/return/delay/pingpong` | `i`/`b`/`f` | `bool` / `i32` / `f32` | `false` | Enable/disable ping-pong channel routing (feedback crosses left/right channels). |
| `/mixer/return/delay/drive` | `f` | `f32` | `0.2` | Tape saturation input drive factor (Tape mode only). Range: `0.0` (clean) to `1.0` (heavy saturation). |
| `/mixer/return/delay/grain_size` | `f` | `f32` | `0.1` | Granular delay grain duration size in seconds. Range: `0.01` to `0.5` (10ms to 500ms). |
| `/mixer/return/delay/density` | `i` | `i32` / `usize` | `4` | Granular delay active grain density count. Range: `1` to `8`. |
| `/mixer/return/delay/spray` | `f` | `f32` | `0.02` | Granular delay randomized spray/jitter start offset in seconds. Range: `0.0` to `0.5` (0ms to 500ms). |
| `/mixer/return/delay/pitch` | `f` | `f32` | `1.0` | Granular delay pitch scaling factor ratio. Range: `0.5` (one octave down) to `2.0` (one octave up). |

### Track FX Chain Controls

Each track features an independent effects chain that can be modulated in real-time via OSC:

| Address | Arguments | Argument Types | Default Value | Reasonable Range / Description |
| :--- | :--- | :--- | :--- | :--- |
| `/track/<id>/fx/filter/position` | `f` | `f32` | `0.0` | DJ filter position. Low-Pass sweep: `-1.0` to `0.0` (20000 Hz down to 20 Hz). High-Pass sweep: `0.0` to `1.0` (20 Hz up to 20000 Hz). Center `0.0` is bypass. |
| `/track/<id>/fx/ring_mod/mode` | `i`/`b` | `i32` / `bool` | `0` (false) | Enable/disable Ring Modulation (0: Off, 1: On). |
| `/track/<id>/fx/ring_mod/source` | `i` | `i32` | `0` | Carrier source. `0` = Sine oscillator, `1` = Self-modulation, `2` = Sidechain input. |
| `/track/<id>/fx/ring_mod/depth` | `f` | `f32` | `0.0` | Wet/dry modulation mix. Range: `0.0` (bypassed) to `1.0` (fully modulated). |
| `/track/<id>/fx/ring_mod/freq` | `f` | `f32` | `440.0` | Carrier oscillator frequency in Hz. Range: `0.1` to `20000.0`. |
| `/track/<id>/fx/distortion/mode` | `i` | `i32` | `0` | Saturation mode. `0` = Bypass, `1` = Tanh, `2` = HardClip, `3` = Wavefold. |
| `/track/<id>/fx/distortion/drive` | `f` | `f32` | `1.0` | Distortion input gain/drive factor. Range: `0.0` to `10.0` (values > 1.0 increase saturation). |
| `/track/<id>/fx/distortion/mix` | `f` | `f32` | `0.0` | Wet/dry distortion mix. Range: `0.0` (dry) to `1.0` (wet). |
| `/track/<id>/fx/compressor/ratio` | `f` | `f32` | `1.0` | Compression ratio. Range: `1.0` (no compression) to `20.0` (heavy compression). |
| `/track/<id>/fx/compressor/threshold`| `f` | `f32` | `-24.0` | Threshold in dB, below which compression is applied. Range: `-60.0` to `0.0`. |
| `/track/<id>/fx/compressor/attack` | `f` | `f32` | `0.01` | Attack time in seconds. Range: `0.0` (instant) to `1.0`. |
| `/track/<id>/fx/compressor/release` | `f` | `f32` | `0.1` | Release time in seconds. Range: `0.01` to `5.0`. |
| `/track/<id>/fx/compressor/makeup` | `f` | `f32` | `0.0` | Makeup gain in dB. Range: `-20.0` to `30.0`. |
| `/track/<id>/fx/compressor/sidechain`| `i`/`b` | `i32` / `bool` | `0` (false) | Enable/disable sidechain compression modulated by external source. |

### Note Control (Available on all Tracks)

| Address | Arguments | Argument Types | Description |
| :--- | :--- | :--- | :--- |
| `/track/<id>/note/on` | `ff` | `[f32, f32]` | `[freq, velocity]` Triggers a note. Frequency range: `20.0` to `20000.0` Hz. Velocity range: `0.0` to `1.0`. |
| `/track/<id>/note/off` | `f` | `[f32]` | `[freq]` Releases a specific frequency, or all notes if no argument is provided. |
| `/track/<id>/note` | `fff` | `[f32, f32, f32]` | `[freq, duration_ms, velocity]` Plays a timed note (Track 0, 4, 6, and 7 only). Duration range: `1.0` to `10000.0` ms. |

---

## 3. DSP Effects Reference

The core DSP library offers several high-quality effects modules:

### 1. Compressor (`aillen_core::dsp::Compressor`)

A sidechainable peak-detecting dynamics processor.

- **Bypassed by default** (Ratio = 1.0).
- **Parameters**:
  - `threshold`: `f32` (dB). Default: `-24.0`. Range: `-60.0` to `0.0`.
  - `ratio`: `f32` (ratio multiplier). Default: `1.0` (transparent). Range: `1.0` to `20.0`. Clamped `.max(1.0)`.
  - `attack`: `f32` (seconds). Default: `0.01` (10ms). Range: `0.0` to `1.0`.
  - `release`: `f32` (seconds). Default: `0.1` (100ms). Range: `0.01` to `5.0`.
  - `makeup_gain`: `f32` (dB). Default: `0.0`. Range: `-20.0` to `30.0`.
- **Sidechaining**: Can compress the signal based on a secondary input channel.

### 2. AM / Ring Modulator (`aillen_core::dsp::AmRingMod`)

A modulation processor supporting multiple carriers.

- **Bypassed by default** (Depth = 0.0).
- **Source selection**:
  - `Sine` (0): Internal sine wave oscillator.
  - `SelfMod` (1): Modulates input with itself.
  - `Sidechain` (2): Modulates input with an external sidechain signal.
- **Parameters**:
  - `depth`: `f32` (modulation wet/dry mix). Default: `0.0` (bypassed). Range: `0.0` to `1.0`.
  - `frequency`: `f32` (Hz carrier frequency). Default: `440.0`. Range: `0.1` to `20000.0`.
  - `ring_mod`: `bool` (true for Ring Modulation, false for Amplitude Modulation). Default: `true`.

### 3. Stereo Delay (`aillen_core::dsp::StereoDelay`)

A dual-mode stereo processor containing:

- **Tape Delay**: Simulated tape-loop delay. Featuring linear fractional-delay interpolation (for pitch glide sweeps), adjustable feedback, warm drive/saturation, and ping-pong routing.
  - **Parameters**:
    - `delay_time`: `f32` (seconds). Default: `0.3`. Range: `0.01` to `3.0` (max capacity 3 seconds).
    - `feedback`: `f32` (feedback ratio). Default: `0.5`. Range: `0.0` to `1.0`.
    - `ping_pong`: `bool` (cross-channel routing). Default: `false`.
    - `drive`: `f32` (soft-clipping distortion). Default: `0.2`. Range: `0.0` to `1.0`.
- **Granular Delay**: Slices incoming audio into overlapping windowed grains.
  - **Parameters**:
    - `delay_time`: `f32` (seconds). Default: `0.3`. Range: `0.0` to `4.0` (max capacity 4 seconds).
    - `grain_size`: `f32` (seconds). Default: `0.1` (100ms). Range: `0.01` to `0.5`.
    - `density`: `usize` (active overlapping grains). Default: `4`. Range: `1` to `8`.
    - `spray`: `f32` (jitter offset in seconds). Default: `0.02`. Range: `0.0` to `0.5`.
    - `pitch`: `f32` (playback ratio / pitch factor). Default: `1.0`. Range: `0.5` to `2.0`.
    - `feedback`: `f32`. Default: `0.4`. Range: `0.0` to `1.0`.

### 4. WaveLoss (`aillen_core::dsp::WaveLoss`)

A zero-crossing wave-dropping distortion processor applied globally at the master output.

- **Bypassed by default** (`drop = 0`).
- **Parameters**:
  - `drop`: `usize` (zero-crossing segments to drop per cycle). Default: `0`. Range: `0` to `outof`.
  - `outof`: `usize` (total zero-crossing segments per cycle). Default: `40`. Range: `1` to `1000`.
  - `mode`: `usize` (`1` = deterministic, `2` = random). Default: `1`.

### 5. Distortion (`aillen_core::dsp::distortion::Distortion`)

A waveshaping distortion processor with configurable drive, wet/dry mix, and multiple modes:

- **Modes**:
  - `Bypass` (0)
  - `Tanh` (1): Soft-clipping hyperbolic tangent saturation.
  - `HardClip` (2): Hard clipping at threshold boundaries.
  - `Wavefold` (3): Sinusoidal wavefolding distortion.
- **Parameters**:
  - `drive`: `f32` (input gain factor). Default: `1.0`. Range: `0.0` to `10.0`.
  - `mix`: `f32` (wet/dry mix). Default: `0.0`. Range: `0.0` to `1.0`.

### 6. LFO (`aillen_core::dsp::lfo::Lfo`)

A modular Low Frequency Oscillator supporting:

- **Waveforms**: Sine (0), Triangle (1), Saw (2), Square (3), and Random Sample & Hold (4).
- **Parameters**:
  - `frequency`: `f32` (Hz). Default: `2.0`. Range: `0.001` to `100.0`. Clamped `.max(0.001)`.
  - `waveform`: `LfoWaveform`. Default: `Sine`.

### 7. Formant Filter (`aillen_core::dsp::filter::formant::FormantFilter`)

A parallel multi-peak bandpass filter modeling human vocal tract resonances. Supports smooth vocal sweeps by morphing a single `vowel` parameter from `0.0` to `1.0` spanning the phonetic vowels [A, E, I, O, U].

- **Parameters**:
  - `vowel`: `f32` (morph position). Default: `0.0`. Range: `0.0` to `1.0`.

### 8. ZDF Ladder Filter (`aillen_core::dsp::filter::ladder::ResonantLadderFilter`)

A stable, Zero-Delay Feedback (ZDF) 4-pole resonant ladder filter mimicking transistor/diode ladder designs. Features resonance gain compensation and a high-pass feedback filter to prevent low-end bass cancellation.

- **Parameters**:
  - `cutoff`: `f32` (cutoff frequency in Hz). Default: `1000.0`. Range: `20.0` to `sample_rate * 0.49`.
  - `resonance`: `f32` (feedback amount). Default: `0.5`. Range: `0.0` to `0.99`.

### 9. Biquad Filter (`aillen_core::dsp::filter::biquad::BiquadFilter`)

A standard 2-pole Biquad IIR Filter implemented using Normalized Transposed Direct Form II to prevent internal clipping errors. Supports LowPass, HighPass, BandPass, and Notch configurations.

- **Parameters**:
  - `cutoff`: `f32` (cutoff frequency in Hz). Default: `1000.0`. Range: `20.0` to `sample_rate / 2.0 - 1.0`.
  - `q_factor`: `f32` (filter resonance Q). Default: `0.707` (flat response). Range: `0.1` to `10.0+`. Clamped `.max(0.1)`.
  - `filter_type`: `FilterType` (LowPass = 0, HighPass = 1, BandPass = 2, Notch = 3).

### 10. DJ Performance Filter (`aillen_core::dsp::filter::dj::DjFilter`)

A performance-oriented filter combining a Low-Pass and High-Pass filter on a single position coordinate parameter:

- **Parameters**:
  - `position`: `f32` (position sweep). Default: `0.0` (bypass). Range: `-1.0` (LP sweep) to `1.0` (HP sweep).
    - **Position < 0.0**: Low-Pass mode (exponentially sweeps cutoff frequency from 20000 Hz down to 20 Hz).
    - **Position > 0.0**: High-Pass mode (exponentially sweeps cutoff frequency from 20 Hz up to 20000 Hz).

### 11. Stereo Limiter (`aillen_core::dsp::Limiter`)

A stereo look-ahead peak limiter used to maximize volume level while preventing digital output clipping:

- **Parameters**:
  - `threshold_gain`: `f32` (pre-limiter gain boost multiplier). Default: `1.0`. Range: `1.0` to `10.0+`.
  - `ceiling`: `f32` (output ceiling limit amplitude). Default: `0.99`. Range: `0.01` to `1.0`.
  - `release_s`: `f32` (release time in seconds). Default: `0.05` (50ms). Range: `0.01` to `1.0`.

### 12. Stereo Panner (`aillen_core::dsp::panner::Panner`)

A stereo positioning node for panning signals across a stereo field. Supports three pan laws:

- **Modes**:
  - `ConstantPowerSin` (0): Keeps acoustic output power identical across all pan positions.
  - `ConstantPowerSqrt` (1): Alternative constant power panning curve.
  - `MidSide` (2): Constant amplitude-based panning.
- **Parameters**:
  - `pan`: `f32` (position). Default: `0.0` (center). Range: `-1.0` (Hard Left) to `1.0` (Hard Right).

### 13. Variable Delay Line (`aillen_core::dsp::delay::variable::VariableDelay`)

A simple, single-channel ring-buffered delay line supporting linear fractional-delay interpolation. It is designed to be a lightweight building block for delay-based effects (such as Chorus, Flanger, and Vibrato).

- **Parameters**:
  - `delay_sec`: `f32` (interpolated delay duration in seconds). Range: `0.0` to maximum buffer capacity capacity.

---

## 4. Instrument-Specific Settings

### Track 0 & 4: TwoOp Synth

| Address | Argument | Argument Types | Default Value | Reasonable Range / Description |
| :--- | :--- | :--- | :--- | :--- |
| `/track/<id>/realtime` | `i` | `i32` | `0` | Monotimbral mode toggle. `0` = Polytimbral (default), `1` = Monotimbral. |
| `/track/<id>/legato` | `i` | `i32` | `1` (true) | Legato mode toggle (mono mode only) to skip envelope re-triggering. `0` = Off, `1` = On. |
| `/track/<id>/mode` | `i` | `i32` | `0` | Synthesis Mode. `0` = Additive, `1` = AM, `2` = RM, `3` = FM (Phase Modulation). |
| `/track/<id>/osc1/waveform` | `i` | `i32` | `1` (Saw) | Operator 1 (Carrier) waveform. `0` = Sine, `1` = Saw, `2` = Square, `3` = Triangle. |
| `/track/<id>/osc2/waveform` | `i` | `i32` | `1` (Saw) | Operator 2 (Modulator) waveform. `0` = Sine, `1` = Saw, `2` = Square, `3` = Triangle. |
| `/track/<id>/mod/params` | `fff` | `[f32, f32, f32]` | `[1.0, 1.0, 0.0]` | Modulator parameters `[index, ratio, detune]`. Index (depth): `0.0` to `20.0`. Ratio (frequency multiplier): `0.1` to `32.0`. Detune: `-10.0` to `10.0` Hz. |
| `/track/<id>/osc1/adsr` | `ffff` | `[f32, f32, f32, f32]` | `[0.01, 0.2, 0.5, 0.5]` | Operator 1 Amplitude ADSR envelope parameters `[A, D, S, R]`. A/D/R (seconds): `0.001` to `10.0`. S (level): `0.0` to `1.0`. |
| `/track/<id>/osc2/adsr` | `ffff` | `[f32, f32, f32, f32]` | `[0.01, 0.2, 0.5, 0.5]` | Operator 2 Modulator ADSR envelope parameters `[A, D, S, R]`. A/D/R (seconds): `0.001` to `10.0`. S (level): `0.0` to `1.0`. |
| `/track/<id>/filter/adsr` | `ffff` | `[f32, f32, f32, f32]` | `[0.05, 0.3, 0.2, 0.5]` | Filter Cutoff modulation envelope parameters `[A, D, S, R]`. A/D/R (seconds): `0.001` to `10.0`. S (level): `0.0` to `1.0`. |
| `/track/<id>/filter/params` | `ffi` | `[f32, f32, i32]` | `[1000.0, 0.707, 0]` | Filter parameters `[cutoff, Q, type]`. Cutoff: `20.0` to `20000.0` Hz. Q (resonance): `0.1` to `10.0+`. Type: `0` = LP, `1` = HP, `2` = BP, `3` = Notch. |
| `/track/<id>/filter/mod` | `bf` | `[bool, f32]` | `[true, 5000.0]` | Cutoff modulation parameters `[enabled, amount]`. Amount (envelope depth): `-20000.0` to `20000.0` Hz. |
| `/track/<id>/feedback`<br>_or_ `/track/<id>/twoop/feedback` | `f` | `f32` | `0.0` | Modulator phase self-feedback intensity. Range: `0.0` to `1.0` (morphs sine to saw/noise). |
| `/track/<id>/wavefold`<br>_or_ `/track/<id>/twoop/wavefold` | `ff` | `[f32, f32]` | `[1.0, 0.0]` | Modulator wavefolder configuration `[gain, mix]`. Gain: `1.0` to `10.0`. Mix (dry/wet): `0.0` to `1.0`. |
| `/track/<id>/noise`<br>_or_ `/track/<id>/twoop/noise` | `ff` | `[f32, f32]` | `[0.0, 0.0]` | Phase noise injection levels `[carrier_noise, modulator_noise]`. Range: `0.0` to `1.0`. |
| `/track/<id>/pitch/sweep`<br>_or_ `/track/<id>/twoop/pitch/sweep` | `ff` | `[f32, f32]` | `[0.0, 0.1]` | Pitch sweep range and decay `[depth_semitones, decay_sec]`. Depth: `-48.0` to `48.0` semitones. Decay: `0.001` to `5.0` seconds. |
| `/track/<id>/lfo`<br>_or_ `/track/<id>/twoop/lfo` | `ifff` | `[i32, f32, f32, f32]` | `[0, 2.0, 0.0, 0.0]` | Voice LFO config `[waveform, speed_hz, mod_index_depth, cutoff_depth]`. Waveform: `0` = Sine, `1` = Tri, `2` = Saw, `3` = Square, `4` = S&H. Speed: `0.001` to `100.0` Hz. Mod Index Depth: `0.0` to `10.0`. Cutoff Depth: `0.0` to `10000.0` Hz. |

### Track 1, 2, 3 & 5: Sampler

Loads audio files (WAV, MP3, FLAC, etc.) and plays them back polyphonically.

| Address | Argument | Argument Types | Default Value | Reasonable Range / Description |
| :--- | :--- | :--- | :--- | :--- |
| `/track/<id>/sample/load` | `s` | `string` | - | Absolute or relative path to load an audio file from disk in real-time. |
| `/track/<id>/sample/select` | `s` | `string` | - | Selects a preloaded sample by its path relative to the `SampleBank` root. |
| `/track/<id>/sample/mode` | `i` | `i32` | `0` | Playback mode. `0` = OneShot (default), `1` = Loop. |
| `/track/<id>/sample/pitch` | `f` | `f32` | `1.0` | Base pitch shifting factor ratio. Range: `0.1` to `10.0`. |
| `/track/<id>/sample/speed` | `f` | `f32` | `1.0` | Base playback speed factor ratio. Range: `0.1` to `10.0`. |
| `/track/<id>/sample/mode/stretch`| `i` | `i32` | `0` | Time stretching mode. `0` = Resample (pitch/speed linked), `1` = Granular (independent pitch/time). |
| `/track/<id>/sample/grain_size` | `f` | `f32` | `40.0` | Granular grain size duration in milliseconds. Range: `5.0` to `500.0` ms. |
| `/track/<id>/sample/overlap` | `i` | `i32` | `4` | Overlapping grain count. Range: `1` to `16`. |
| `/track/<id>/filter` | `f` | `f32` | `0.0` | Sampler output channel DJ filter position. Low-Pass sweep: `-1.0` to `0.0` (20000 Hz down to 20 Hz). High-Pass sweep: `0.0` to `1.0` (20 Hz up to 20000 Hz). Center `0.0` is bypass. |
| `/track/<id>/sample/slice/mode` | `i`/`b` | `i32` / `bool` | `0` (false) | Enable/disable sample slice playback mode. |
| `/track/<id>/sample/slice/count` | `i` | `i32` | `1` | Total number of slices to segment the sample buffer into. Range: `1` to `128`. |
| `/track/<id>/sample/slice/select` | `i` | `i32` | `0` | Active slice index selector. Range: `0` to `count - 1`. |
| `/track/<id>/sample/slice/stutter` | `i` | `i32` | `1` | Stutter repetition count for slice re-triggering. Range: `1` to `64`. |

### Track 6: Synth303 (Acid Bass Synth)

A monophonic, legato-enabled synthesizer mimicking the Roland TB-303.

| Address | Argument | Argument Types | Default Value | Reasonable Range / Description |
| :--- | :--- | :--- | :--- | :--- |
| `/track/6/303/waveform` | `i` | `i32` | `1` (Saw) | Oscillator waveform. `0` = Sine, `1` = Saw, `2` = Square, `3` = Triangle. |
| `/track/6/303/amp/adsr` | `ffff` | `[f32, f32, f32, f32]` | `[0.002, 0.3, 0.1, 0.2]` | Amplitude ADSR envelope parameters `[A, D, S, R]`. A/D/R (seconds): `0.001` to `10.0`. S (level): `0.0` to `1.0`. |
| `/track/6/303/filter/adsr` | `ffff` | `[f32, f32, f32, f32]` | `[0.002, 0.25, 0.05, 0.2]` | Filter Cutoff ADSR envelope parameters `[A, D, S, R]`. A/D/R (seconds): `0.001` to `10.0`. S (level): `0.0` to `1.0`. |
| `/track/6/303/pitch/adsr` | `ffff` | `[f32, f32, f32, f32]` | `[0.002, 0.1, 0.0, 0.1]` | Pitch ADSR envelope parameters `[A, D, S, R]`. A/D/R (seconds): `0.001` to `10.0`. S (level): `0.0` to `1.0`. |
| `/track/6/303/filter/params` | `ff` | `[f32, f32]` | `[300.0, 0.75]` | Filter params `[cutoff, resonance]`. Cutoff: `20.0` to `20000.0` Hz. Resonance (feedback): `0.0` to `0.99` (0.75+ for squelchy self-oscillation). |
| `/track/6/303/filter/mod` | `f` | `f32` | `3000.0` | Cutoff envelope modulation depth. Range: `-20000.0` to `20000.0` Hz. |
| `/track/6/303/pitch/mod` | `f` | `f32` | `0.0` | Pitch envelope modulation depth. Range: `-10000.0` to `10000.0` Hz. |
| `/track/6/303/pwm/params` | `fff` | `[f32, f32, f32]` | `[0.5, 1.0, 0.0]` | Pulse width modulation params `[pw, rate, depth]`. PW: `0.05` to `0.95`. Rate: `0.001` to `50.0` Hz. Depth: `0.0` to `1.0`. |
| `/track/6/303/glide` | `f` | `f32` | `0.1` | Glide/Portamento time in seconds. Range: `0.0` (no glide) to `2.0` seconds. |
| `/track/6/303/legato` | `i`/`b` | `i32` / `bool` | `1` (true) | Legato slide toggle (does not re-trigger envelopes on overlapping notes). `0` = Off, `1` = On. |

### Track 7: SynthHubass (Rave & Heavy Bass Synth)

A massive, versatile synthesizer designed for heavy basslines and rave textures. It features a configurable detuned unison generator, a dedicated filter-bypassed sub oscillator, parallel stereo multi-mode filters (ZDF Ladder, ZDF Biquad, and Formant vowel filter), modular LFO modulation, waveshaping saturation/drive, and a stereo chorus unit.

| Address | Argument | Argument Types | Default Value | Reasonable Range / Description |
| :--- | :--- | :--- | :--- | :--- |
| `/track/7/hubass/amp/adsr` | `ffff` | `[f32, f32, f32, f32]` | `[0.05, 0.2, 0.7, 0.3]` | Amplitude ADSR envelope parameters `[A, D, S, R]`. A/D/R (seconds): `0.001` to `10.0`. S (level): `0.0` to `1.0`. |
| `/track/7/hubass/filter/params` | `ffff` | `[f32, f32, f32, f32]` | `[1.333, 800.0, 1.0, 0.4]` | Cutoff envelope params `[start_mult, end_cf, decay, resonance]`. `start_mult` (multiplier): `0.1` to `10.0`. `end_cf` (target cutoff frequency): `20.0` to `20000.0` Hz. `decay` (seconds): `0.001` to `10.0` seconds. Resonance (feedback): `0.0` to `0.99`. |
| `/track/7/hubass/osc/unison` | `fffi` | `[f32, f32, f32, i32]` | `[0.0, 0.035, 0.8, 5]` | Detuned unison config `[waveform, detune, spread, voices]`. Waveform: `0.0` = Saw, `1.0` = Square, `2.0` = Triangle. Detune: `0.0` to `0.2` (depth). Spread (stereo width): `0.0` to `1.0`. Voices: `1` to `7`. |
| `/track/7/hubass/osc/sub` | `iif` | `[i32, i32, f32]` | `[0, -1, 0.7]` | Mono sub-bass oscillator config `[waveform, octave, gain]`. Waveform: `0` = Sine, `1` = Triangle, `2` = Square. Octave offset: `-1` or `-2`. Gain: `0.0` to `2.0`. |
| `/track/7/hubass/osc/noise` | `f` | `f32` | `0.05` | Noise generator gain level. Range: `0.0` to `1.0`. |
| `/track/7/hubass/filter/mode` | `i` | `i32` | `0` | Filter Mode. `0` = ZDF LowPass, `1` = ZDF BandPass, `2` = Formant vowel morph filter. |
| `/track/7/hubass/drive/mode` | `iff` | `[i32, f32, f32]` | `[1, 2.0, 0.5]` | Saturation/distortion config `[mode, gain, mix]`. Mode: `0` = Bypass, `1` = Tanh, `2` = HardClip, `3` = Wavefold. Gain (drive boost): `0.0` to `10.0`. Mix (dry/wet): `0.0` to `1.0`. |
| `/track/7/hubass/lfo/1` | `ifff` | `[i32, f32, f32, f32]` | `[0, 1.5, 0.0, 0.0]` | Modular LFO 1 config `[waveform, speed, cutoff_depth, pitch_depth]`. Waveform: `0` = Sine, `1` = Tri, `2` = Saw, `3` = Square, `4` = S&H. Speed: `0.001` to `100.0` Hz. Cutoff Depth (mod depth in Hz): `0.0` to `10000.0` Hz. Pitch Depth: `0.0` to `12.0` semitones. |
| `/track/7/hubass/chorus/params` | `ff` | `[f32, f32]` | `[0.5, 0.5]` | Stereo chorus parameters `[mix, depth]`. Mix: `0.0` (dry) to `1.0` (wet). Depth: `0.0` to `1.0`. |
| `/track/7/hubass/legato` | `i`/`b` | `i32` / `bool` | `1` (true) | Legato slide toggle. `0` = Off, `1` = On. |
| `/track/7/hubass/gain` | `f` | `f32` | `1.0` | Output channel gain volume multiplier. Range: `0.0` to `5.0`. |

---

## 5. SampleBank Loading

Aillen can automatically scan a directory on startup (defaulting to `~/Desktop/KairosSamples` if not specified) and preload all found `.wav`, `.flac`, `.mp3`, `.aif`, and `.aiff` files.

You can trigger these preloaded buffers instantly via OSC using `/track/1/sample/select "subfolder/myloop.wav"`.

---

## 6. Atomic Updates (Bundles)

To update multiple parameters and trigger notes simultaneously without artifacts, use **OSC Bundles**.

**Python (python-osc) Example:**

```python
from pythonosc import udp_client, osc_bundle_builder, osc_message_builder

client = udp_client.SimpleUDPClient("127.0.0.1", 8000)
bundle = osc_bundle_builder.OscBundleBuilder(osc_bundle_builder.IMMEDIATELY)

# Switch Track 0 to FM mode (index 3)
msg = osc_message_builder.OscMessageBuilder(address="/track/0/mode")
msg.add_arg(3) 
bundle.add_content(msg.build())

# Play note on Track 0
msg = osc_message_builder.OscMessageBuilder(address="/track/0/note/on")
msg.add_arg(220.0) # Hz
msg.add_arg(0.7)   # velocity
bundle.add_content(msg.build())

client.send(bundle.build())
```
