# Aillen

Opinionated, feature-incomplete audio engine, DSP library, and live synthesizers.

## Project Structure

This project is set up as a Cargo Workspace containing:

- `aillen-core`: A modular DSP library hosting mathematical primitives, oscillators, filters, ADSR envelopes, and instrument implementations based on a generic `Voice` trait.
- `aillen-cli`: A standalone performance synthesizer that wraps `aillen-core` with real-time audio (`cpal`) and an asynchronous UDP OSC server mapped via lock-free channels (`crossbeam-channel`).

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

## 2. OSC API (TwoOp Instrument)

The Aillen CLI listens for OSC messages over UDP (default port 8000). Currently, it hosts the **TwoOp** instrument, and all messages for it must be prepended with `/two_op/`.

### Timbral Behavior: Polytimbral vs Monotimbral
By default, the synth is **Polytimbral**. This means each note "captures" the current master patch when it is triggered. If you change a parameter (like cutoff) while notes are already ringing, those existing notes will **not** change.

To enable live tweaking of active notes (Monotimbral mode), send `/two_op/realtime i 1`.

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/two_op/realtime` | `i` | 0: **Polytimbral** (default). 1: **Monotimbral** (Global updates). |

### Note Control

| Address | Arguments | Description |
| :--- | :--- | :--- |
| `/two_op/note` | `fff` | `[freq, duration_ms, velocity]` Plays a timed note. |
| `/two_op/note/on` | `ff` | `[freq, velocity]` Triggers a note. |
| `/two_op/note/off` | `f` | `[freq]` Releases a specific frequency, or all notes if no arg. |
| `/two_op/legato` | `i` | 0/1: Enables legato (mono mode only) to skip envelope re-triggering. |

### Synthesis Engine

| Address | Argument | Description |
| :--- | :--- | :--- |
| `/two_op/mode` | `i` | 0: Additive, 1: AM, 2: **RM (Ring Mod)**, 3: FM |
| `/two_op/osc1/waveform` | `i` | 0: Sine, 1: Saw, 2: Square, 3: Triangle |
| `/two_op/osc2/waveform` | `i` | 0: Sine, 1: Saw, 2: Square, 3: Triangle |
| `/two_op/mod/params` | `fff` | `[index, ratio, detune]` FM/AM/RM intensity and tuning. |

### Envelopes & Filter

| Address | Arguments | Description |
| :--- | :--- | :--- |
| `/two_op/osc1/adsr` | `ffff` | `[A, D, S, R]` Amplitude envelope (Sec, Sec, 0.0-1.0, Sec). |
| `/two_op/osc2/adsr` | `ffff` | `[A, D, S, R]` Modulator envelope. |
| `/two_op/filter/adsr` | `ffff` | `[A, D, S, R]` Cutoff modulation envelope. |
| `/two_op/filter/params` | `ffi` | `[cutoff, Q, type]` (Type: 0:LP, 1:HP, 2:BP, 3:Notch). |
| `/two_op/filter/mod` | `bf` | `[enabled, amount]` Enable envelope modulation and set depth (Hz). |

---

## 3. Atomic Updates (Bundles)

To change a patch and trigger a note simultaneously without artifacts, use **OSC Bundles**.

**Python (python-osc) Example:**

```python
from pythonosc import udp_client, osc_bundle_builder, osc_message_builder

client = udp_client.SimpleUDPClient("127.0.0.1", 8000)
bundle = osc_bundle_builder.OscBundleBuilder(osc_bundle_builder.IMMEDIATELY)

# Switch to RM mode (index 2)
msg = osc_message_builder.OscMessageBuilder(address="/two_op/mode")
msg.add_arg(2) 
bundle.add_content(msg.build())

# Play note
msg = osc_message_builder.OscMessageBuilder(address="/two_op/note")
msg.add_arg(220.0) # Hz
msg.add_arg(500.0) # ms
msg.add_arg(0.7)   # velocity
bundle.add_content(msg.build())

client.send(bundle.build())
```
