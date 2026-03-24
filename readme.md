# Aillen

Opinionated feature incomplete audio engine, dsp lib and live synthesizers.

## Project Structure

This project is set up as a Cargo Workspace with modern Rust 1.92 standards, containing:
- `aillen-core`: A pure, dependency-free DSP library hosting mathematical primitives, oscillators, filters, ADSR envelopes, and `SynthVoice`/`PolySynth` assemblies.
- `aillen-cli`: A standalone performance synthesizer that wraps `aillen-core` with real-time audio (`cpal`) and an asynchronous UDP OSC server mapped via lock-free channels (`crossbeam-channel`).

## Compiling

To build the entire workspace optimally, use the `--workspace` flag:

```bash
cargo build --workspace --release
```

To quickly check for compilation/syntax errors without producing a binary:

```bash
cargo check --workspace
```

## Running the Live Synthesizer

You can run the CLI synth directly. It connects to your default OS audio device and begins listening for UDP OSC messages on port 8000. It defaults to 8-voice polyphony!

```bash
cargo run --release -p aillen-cli
```

*(Options: `--port 9000` to change the UDP port, `--voices 8` to set polyphony count. Setting `--voices 1` creates a purely monophonic synth).*

## Testing with OSC

Once the application is running and the audio thread is active, you can interact with the engine using any OSC-compliant software (like Max/MSP, PureData, TouchOSC, or the `oscsend` CLI utility) by sending messages to `127.0.0.1:8000`.

- **Trigger a Note On:**
  - Address: `/note/on`
  - Arguments: `[frequency (f32), velocity (f32)]`
  - Example: `oscsend localhost 8000 /note/on ff 440.0 1.0`
  
- **Trigger a Note Off:**
  - Address: `/note/off`
  - Arguments: `[frequency (f32)]` (optional, ommiting it turns off all active notes)
  - Example: `oscsend localhost 8000 /note/off f 440.0`

- **Trigger a Timed Note (Auto Release):**
  - Address: `/note`
  - Arguments: `[frequency (f32), duration_ms (f32), velocity (f32)]`
  - Example: `oscsend localhost 8000 /note fff 440.0 500.0 1.0` (Plays for 500ms then triggers release automatically)

- **Enable/Disable Legato (Mono Mode Only):**
  - Address: `/legato`
  - Arguments: `[enabled (bool or int or float)]`
  - Example: `oscsend localhost 8000 /legato i 1` (Enables legato so sustained notes don't re-trigger envelopes when playing monophonically)
