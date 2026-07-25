# Aillen

Opinionated, feature-incomplete audio engine, DSP library, and live synthesizers.

## Project Structure

This project is set up as a Cargo Workspace containing:

- `aillen-core`: A modular DSP library hosting mathematical primitives, oscillators, filters (including standard Biquad and DJ performance filters), ADSR envelopes, sidechainable dynamic effects (Compressor, AM/Ring Modulator), a stereo delay (Tape and Granular modes), a sequential track `FxChain`, and instrument implementations (including a 2-operator FM synth, a sampler, and a sample bank).
- `aillen-cli`: A standalone performance synthesizer that wraps `aillen-core` with real-time stereo audio (`cpal`) and an asynchronous UDP OSC server mapped via lock-free channels (`crossbeam-channel`).

---

## Signal Flow Graph

The following ASCII diagram illustrates the audio signal path from the instruments to the final stereo hardware output, highlighting the track-level `FxChain` inserts, the send/return routing, and the master bus:

```
                            +---------------------------------------+
                            |              TRACK (0 or 1)           |
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
                                 |Distortn | |Distortn |
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

### Audio Device Management

If you have multiple audio interfaces, use these flags to select the correct one:

```bash
# List all available output devices and their indices
cargo run -p aillen-cli -- --list-devices

# Start using a specific device by index (e.g., index 2)
cargo run -p aillen-cli -- --device-index 2
```

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

| Address | Arguments | Description |
| :--- | :--- | :--- |
| `/mixer/master/volume` | `f` | Master output volume gain factor (e.g., 0.0 - 1.0+). |
| `/mixer/master/filter` | `f` | Master output DJ filter position from `-1.0` (LP sweep) to `1.0` (HP sweep). Center `0.0` is bypass. |
| `/mixer/master/waveloss/drop` | `i` | Master WaveLoss drop segments count. |
| `/mixer/master/waveloss/outof` | `i` | Master WaveLoss total segments per cycle. |
| `/mixer/master/waveloss/mode` | `i` | Master WaveLoss mode (1 = deterministic, 2 = random). |
| `/track/<id>/volume` | `f` | Individual track volume gain factor. |
| `/track/<id>/pan` | `f` | Track panning position from `-1.0` (Hard Left) to `1.0` (Hard Right). |
| `/track/<id>/mute` | `i`/`b` | Mute (1 or true) or unmute (0 or false) the track. |
| `/track/<id>/send/delay` | `f` | Send level (0.0 to 1.0) of this track's signal to the return delay track. |

### Track FX Chain Controls

Each track features an independent effects chain that can be modulated in real-time via OSC:

| Address | Arguments | Description |
| :--- | :--- | :--- |
| `/track/<id>/fx/filter/position` | `f` | DJ filter position from `-1.0` (Low-Pass) to `1.0` (High-Pass). `0.0` is bypass. |
| `/track/<id>/fx/ring_mod/mode` | `i`/`b` | Enable/disable Ring Modulation (0: Off, 1: On). |
| `/track/<id>/fx/ring_mod/source` | `i` | Carrier source (0: Sine oscillator, 1: Self-modulation, 2: Sidechain input). |
| `/track/<id>/fx/ring_mod/depth` | `f` | Wet/dry modulation mix (0.0 to 1.0). |
| `/track/<id>/fx/ring_mod/freq` | `f` | Carrier oscillator frequency in Hz. |
| `/track/<id>/fx/distortion/mode` | `i` | Saturation mode (0: Bypass, 1: Tanh, 2: HardClip, 3: Wavefold). |
| `/track/<id>/fx/distortion/drive` | `f` | Distortion input gain/drive factor (0.0 to 10.0). |
| `/track/<id>/fx/distortion/mix` | `f` | Wet/dry distortion mix (0.0 to 1.0). |
| `/track/<id>/fx/compressor/ratio` | `f` | Compression ratio (1.0 to 20.0). |
| `/track/<id>/fx/compressor/threshold`| `f` | Threshold in dB (-60.0 to 0.0). |
| `/track/<id>/fx/compressor/attack` | `f` | Attack time in seconds. |
| `/track/<id>/fx/compressor/release` | `f` | Release time in seconds. |
| `/track/<id>/fx/compressor/makeup` | `f` | Makeup gain in dB. |
| `/track/<id>/fx/compressor/sidechain`| `i`/`b` | Enable sidechain compression modulated by external source. |

### Note Control (Available on all Tracks)

| Address | Arguments | Description |
| :--- | :--- | :--- |
| `/track/<id>/note/on` | `ff` | `[freq, velocity]` Triggers a note. |
| `/track/<id>/note/off` | `f` | `[freq]` Releases a specific frequency, or all notes if no arg. |
| `/track/<id>/note` | `fff` | `[freq, duration_ms, velocity]` Plays a timed note (Track 0, 4, 6, and 7 only). |

---

## 3. DSP Effects Reference

The core DSP library offers several high-quality effects modules:

### 1. Compressor (`aillen_core::dsp::Compressor`)

A sidechainable peak-detecting dynamics processor.
- **Bypassed by default** (Ratio = 1.0).
- **Controls**: Threshold (dB), Ratio, Attack (seconds), Release (seconds), Makeup Gain (dB).
- **Sidechaining**: Can compress the signal based on a secondary input channel.

### 2. AM / Ring Modulator (`aillen_core::dsp::AmRingMod`)

A modulation processor supporting multiple carriers.
- **Bypassed by default** (Depth = 0.0).
- **Source selection**:
  - `Sine`: Internal sine wave oscillator (uses adjustable Frequency).
  - `SelfMod`: Modulates input with itself.
  - `Sidechain`: Modulates input with an external sidechain signal.
- **Controls**: Depth/Mix (0.0 to 1.0), Frequency (Hz), Ring Mod mode (true/false).

### 3. Stereo Delay (`aillen_core::dsp::StereoDelay`)

A dual-mode stereo processor containing:
- **Tape Delay**: Simulated tape-loop delay. Featuring linear fractional-delay interpolation (for pitch glide sweeps), adjustable feedback, warm drive/saturation, and ping-pong routing.
- **Granular Delay**: Slices incoming audio into overlapping windowed grains. Featuring configurable grain size (10ms–500ms), active density (1–8 active grains), pitch playback ratios (0.5x–2.0x), and randomized spray/jitter offsets.

### 4. WaveLoss (`aillen_core::dsp::WaveLoss`)

A zero-crossing wave-dropping distortion processor applied globally at the master output.
- **Bypassed by default** (`drop = 0`).
- **Controls**: Drop count, Outof cycle total segments, Mode (1 = deterministic, 2 = random).

### 5. Distortion (`aillen_core::dsp::distortion::Distortion`)

A waveshaping distortion processor with configurable drive, wet/dry mix, and multiple modes:
- `Bypass` (0)
- `Tanh` (1): Soft-clipping hyperbolic tangent saturation.
- `HardClip` (2): Hard clipping at threshold boundaries.
- `Wavefold` (3): Sinusoidal wavefolding distortion.

### 6. LFO (`aillen_core::dsp::lfo::Lfo`)

A modular Low Frequency Oscillator supporting:
- Sine (0), Triangle (1), Saw (2), Square (3), and Random Sample & Hold (4) waveforms.

### 7. Formant Filter (`aillen_core::dsp::filter::formant::FormantFilter`)

A parallel multi-peak bandpass filter modeling human vocal tract resonances. Supports smooth vocal sweeps by morphing a single `vowel` parameter from `0.0` to `1.0` spanning the phonetic vowels [A, E, I, O, U].

### 8. ZDF Ladder Filter (`aillen_core::dsp::filter::ladder::ResonantLadderFilter`)

A stable, Zero-Delay Feedback (ZDF) 4-pole resonant ladder filter mimicking transistor/diode ladder designs. Features resonance gain compensation and a high-pass feedback filter to prevent low-end bass cancellation.

---

## 4. Instrument-Specific Settings

### Track 0 & 4: TwoOp Synth

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/track/<id>/realtime` | `i` | 0: **Polytimbral** (default). 1: **Monotimbral** (Global updates). |
| `/track/<id>/legato` | `i` | 0/1: Enables legato (mono mode only) to skip envelope re-triggering. |
| `/track/<id>/mode` | `i` | 0: Additive, 1: AM, 2: RM, 3: FM (PM implementation) |
| `/track/<id>/osc1/waveform` | `i` | 0: Sine, 1: Saw, 2: Square, 3: Triangle |
| `/track/<id>/osc2/waveform` | `i` | 0: Sine, 1: Saw, 2: Square, 3: Triangle |
| `/track/<id>/mod/params` | `fff` | `[index, ratio, detune]` FM/AM/RM intensity and tuning. |
| `/track/<id>/osc1/adsr` | `ffff` | `[A, D, S, R]` Amplitude envelope (Sec, Sec, 0.0-1.0, Sec). |
| `/track/<id>/osc2/adsr` | `ffff` | `[A, D, S, R]` Modulator envelope. |
| `/track/<id>/filter/adsr` | `ffff` | `[A, D, S, R]` Cutoff modulation envelope. |
| `/track/<id>/filter/params` | `ffi` | `[cutoff, Q, type]` (Type: 0:LP, 1:HP, 2:BP, 3:Notch). |
| `/track/<id>/filter/mod` | `bf` | `[enabled, amount]` Enable envelope modulation and set depth (Hz). |
| `/track/<id>/feedback` | `f` | `[feedback]` Modulator self-feedback amount (0.0 to 1.0) to morph sines to saws/noise. |
| `/track/<id>/wavefold` | `ff` | `[gain, mix]` Modulator wavefolder input gain (1.0 to 10.0) and dry/wet mix (0.0 to 1.0). |
| `/track/<id>/noise` | `ff` | `[carrier_noise, modulator_noise]` Phase noise injection amounts (0.0 to 1.0) for glitch textures. |
| `/track/<id>/pitch/sweep` | `ff` | `[depth_semitones, decay_sec]` Pitch envelope sweep range (-48 to +48 semitones) and decay time. |
| `/track/<id>/lfo` | `ifff` | `[waveform, speed_hz, mod_index_depth, cutoff_depth]` Voice LFO config (Waveform: 0:Sine, 1:Tri, 2:Saw, 3:Square, 4:S&H). |

### Track 1, 2, 3 & 5: Sampler

Loads audio files (WAV, MP3, FLAC, etc.) and plays them back polyphonically.

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/track/<id>/sample/load` | `s` | `[path]` Loads an audio file from disk into the sampler in real-time. |
| `/track/<id>/sample/select` | `s` | `[name]` Selects a preloaded sample by its relative path inside the `SampleBank`. |
| `/track/<id>/sample/mode` | `i` | `[mode]` 0: OneShot (default), 1: Loop. |
| `/track/<id>/sample/pitch` | `f` | `[ratio]` Base pitch shifting factor (default 1.0). |
| `/track/<id>/sample/speed` | `f` | `[ratio]` Base playback speed factor (default 1.0). |
| `/track/<id>/sample/mode/stretch`| `i` | `[stretch_mode]` 0: Resample (default), 1: Granular (independent pitch/time). |
| `/track/<id>/sample/grain_size` | `f` | `[size_ms]` Granular grain size duration in milliseconds (default 40.0). |
| `/track/<id>/sample/overlap` | `i` | `[overlap]` Overlapping grain count from 1 to 16 (default 4). |
| `/track/<id>/filter` | `f` | Sampler output channel DJ filter position from `-1.0` (LP) to `1.0` (HP). Center `0.0` is bypass. |
| `/track/<id>/sample/slice/mode` | `i`/`b` | Enable/disable slice-playback mode. |
| `/track/<id>/sample/slice/count` | `i` | Total number of slices to segment the sample buffer into. |
| `/track/<id>/sample/slice/select` | `i` | Select active slice index. |
| `/track/<id>/sample/slice/stutter` | `i` | Stutter repetition count for slice re-triggering. |

### Track 6: Synth303 (Acid Bass Synth)

A monophonic, legato-enabled synthesizer mimicking the Roland TB-303.

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/track/6/303/waveform` | `i` | Oscillator waveform. 0: Sine, 1: Saw (default), 2: Square, 3: Triangle. |
| `/track/6/303/amp/adsr` | `ffff` | `[A, D, S, R]` Amplitude envelope parameters in seconds (sustain is 0.0-1.0). |
| `/track/6/303/filter/adsr` | `ffff` | `[A, D, S, R]` Filter cutoff envelope parameters. |
| `/track/6/303/pitch/adsr` | `ffff` | `[A, D, S, R]` Pitch envelope parameters. |
| `/track/6/303/filter/params` | `ff` | `[cutoff, resonance]` Set base cutoff frequency (Hz) and resonance (0.0 to 1.0). |
| `/track/6/303/filter/mod` | `f` | Cutoff envelope modulation depth (Hz). |
| `/track/6/303/pitch/mod` | `f` | Pitch envelope modulation depth (Hz). |
| `/track/6/303/pwm/params` | `fff` | `[pw, rate, depth]` Set pulse width (0.05-0.95), PWM LFO frequency (Hz), and PWM depth (0.0-1.0). |
| `/track/6/303/glide` | `f` | Glide/Portamento time in seconds (default 0.1s). |
| `/track/6/303/legato` | `i`/`b` | Enable/disable legato (0: Off, 1: On). Defaults to On. |

### Track 7: SynthHubass (Rave & Heavy Bass Synth)

A massive, versatile synthesizer designed for heavy basslines and rave textures. It features a configurable detuned unison generator, a dedicated filter-bypassed sub oscillator, parallel stereo multi-mode filters (ZDF Ladder, ZDF Biquad, and Formant vowel filter), modular LFO modulation, waveshaping saturation/drive, and a stereo chorus unit.

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/track/7/hubass/amp/adsr` | `ffff` | `[A, D, S, R]` Amplitude envelope parameters in seconds. |
| `/track/7/hubass/filter/params` | `ffff` | `[start_mult, end_cf, decay, resonance]` Cutoff envelope start frequency multiplier, target base cutoff (Hz), exponential decay rate (seconds), and resonance (0.0 to 1.0). |
| `/track/7/hubass/osc/unison` | `fffi` | `[waveform (0:Saw, 1:Square, 2:Triangle), detune (0.0-0.2), spread (0.0-1.0), voices (1-7)]` Detuned unison generator config. |
| `/track/7/hubass/osc/sub` | `iif` | `[waveform (0:Sine, 1:Triangle, 2:Square), octave (-1 or -2), gain (0.0-2.0)]` Filter-bypassed, clean mono sub-bass oscillator config. |
| `/track/7/hubass/osc/noise` | `f` | `[gain]` Noise generator gain level (0.0 to 1.0). |
| `/track/7/hubass/filter/mode` | `i` | `[mode]` Filter mode (0: ZDF LP, 1: ZDF BP, 2: Formant vowel morph). |
| `/track/7/hubass/drive/mode` | `iff` | `[mode (0:Bypass, 1:Tanh, 2:HardClip, 3:Wavefold), gain (0.0-10.0), mix (0.0-1.0)]` Saturation/waveshaping distortion. |
| `/track/7/hubass/lfo/1` | `ifff` | `[waveform (0:Sine, 1:Tri, 2:Saw, 3:Square, 4:S&H), speed (Hz), cutoff_depth, pitch_depth]` Modular LFO routing. |
| `/track/7/hubass/chorus/params` | `ff` | `[mix, depth]` Stereo chorus mix and LFO depth. |
| `/track/7/hubass/legato` | `i`/`b` | Enable/disable legato monophonic sliding (0: Off, 1: On). |
| `/track/7/hubass/gain` | `f` | Master output gain multiplier (0.0 to 5.0). Defaults to 1.0. |

---

## 5. SampleBank Loading

Aillen can automatically scan a directory on startup (defaulting to `~/Desktop/KairosSamples` if not specified) and preload all found `.wav`, `.flac`, and `.mp3` files.

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
