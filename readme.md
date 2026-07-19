# Aillen

Opinionated, feature-incomplete audio engine, DSP library, and live synthesizers.

## Project Structure

This project is set up as a Cargo Workspace containing:

- `aillen-core`: A modular DSP library hosting mathematical primitives, oscillators, filters, ADSR envelopes, a stereo mixer, and instrument implementations (including a 2-operator FM synth and a sampler).
- `aillen-cli`: A standalone performance synthesizer that wraps `aillen-core` with real-time stereo audio (`cpal`) and an asynchronous UDP OSC server mapped via lock-free channels (`crossbeam-channel`).

---

## 1. CLI Usage

### Compiling

To build the entire workspace optimally, use the `--release` flag:

```bash
cargo build --workspace --release
```

### Starting the Synth

```bash
# Start with default settings (8 voices, port 8000)
cargo run -p aillen-cli --release

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

The engine now supports a stereo Mixer with exactly two instrument tracks:
- **Track 0**: `TwoOp` (FM Synth)
- **Track 1**: `Sampler` (Sample playback engine with multi-format support via Symphonia)

All OSC messages must target the appropriate track path (`/track/<id>/`) or mixer path (`/mixer/`).

### Mixer & General Track Controls

| Address | Arguments | Description |
| :--- | :--- | :--- |
| `/mixer/master/volume` | `f` | Master output volume gain factor (e.g., 0.0 - 1.0+). |
| `/track/<id>/volume` | `f` | Individual track volume gain factor. |
| `/track/<id>/pan` | `f` | Track panning position from `-1.0` (Hard Left) to `1.0` (Hard Right). |
| `/track/<id>/mute` | `i`/`b` | Mute (1 or true) or unmute (0 or false) the track. |

### Note Control (Available on both Tracks)

| Address | Arguments | Description |
| :--- | :--- | :--- |
| `/track/<id>/note/on` | `ff` | `[freq, velocity]` Triggers a note. |
| `/track/<id>/note/off` | `f` | `[freq]` Releases a specific frequency, or all notes if no arg. |
| `/track/<id>/note` | `fff` | `[freq, duration_ms, velocity]` Plays a timed note (Track 0 only). |

---

## 3. Instrument-Specific Settings

### Track 0: TwoOp Synth

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/track/0/realtime` | `i` | 0: **Polytimbral** (default). 1: **Monotimbral** (Global updates). |
| `/track/0/legato` | `i` | 0/1: Enables legato (mono mode only) to skip envelope re-triggering. |
| `/track/0/mode` | `i` | 0: Additive, 1: AM, 2: RM, 3: FM |
| `/track/0/osc1/waveform` | `i` | 0: Sine, 1: Saw, 2: Square, 3: Triangle |
| `/track/0/osc2/waveform` | `i` | 0: Sine, 1: Saw, 2: Square, 3: Triangle |
| `/track/0/mod/params` | `fff` | `[index, ratio, detune]` FM/AM/RM intensity and tuning. |
| `/track/0/osc1/adsr` | `ffff` | `[A, D, S, R]` Amplitude envelope (Sec, Sec, 0.0-1.0, Sec). |
| `/track/0/osc2/adsr` | `ffff` | `[A, D, S, R]` Modulator envelope. |
| `/track/0/filter/adsr` | `ffff` | `[A, D, S, R]` Cutoff modulation envelope. |
| `/track/0/filter/params` | `ffi` | `[cutoff, Q, type]` (Type: 0:LP, 1:HP, 2:BP, 3:Notch). |
| `/track/0/filter/mod` | `bf` | `[enabled, amount]` Enable envelope modulation and set depth (Hz). |

### Track 1: Sampler

Loads audio files (WAV, MP3, FLAC, etc.) and plays them back polyphonically.

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/track/1/sample/load` | `s` | `[path]` Loads an audio file from disk into the sampler. |
| `/track/1/sample/mode` | `i` | `[mode]` 0: OneShot (default), 1: Loop. |
| `/track/1/sample/pitch` | `f` | `[ratio]` Base pitch shifting factor (default 1.0). |
| `/track/1/sample/speed` | `f` | `[ratio]` Base playback speed factor (default 1.0). |

---

## 4. Atomic Updates (Bundles)

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
