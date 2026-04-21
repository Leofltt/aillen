use aillen_core::dsp::{AudioNode, filter::FilterType, oscillator::Waveform};
use aillen_core::synth::two_op::{two_op::TwoOpSynth, SynthMode};
use anyhow::Result;
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Receiver;

pub enum AudioMessage {
    TwoOp(TwoOpMessage),
}

pub enum TwoOpMessage {
    NoteOn { freq: f32, vel: f32 },
    NoteOff { freq: f32 },
    NoteOffAll,
    TimedNote { freq: f32, vel: f32, duration_ms: f32 },
    SetLegato { enabled: bool },
    SetRealtimeUpdate { enabled: bool },
    SetMode { mode: SynthMode },
    SetOsc1Waveform { waveform: Waveform },
    SetOsc2Waveform { waveform: Waveform },
    SetOsc1Adsr { a: f32, d: f32, s: f32, r: f32 },
    SetOsc2Adsr { a: f32, d: f32, s: f32, r: f32 },
    SetFilterAdsr { a: f32, d: f32, s: f32, r: f32 },
    SetFilterParams { cutoff: f32, q: f32, filter_type: FilterType },
    SetFilterMod { enabled: bool, amount: f32 },
    SetModulationParams { index: f32, ratio: f32, detune: f32 },
}

pub fn list_audio_devices() -> Result<()> {
    let host = cpal::default_host();
    let devices = host.output_devices()?;
    println!("Available Audio Devices:");
    for (i, device) in devices.enumerate() {
        println!("{}. {}", i, device.name().unwrap_or_else(|_| "Unknown".to_string()));
    }
    Ok(())
}

pub fn start_audio_thread(rx: Receiver<AudioMessage>, num_voices: usize, device_index: Option<usize>) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    
    let device = if let Some(index) = device_index {
        host.output_devices()?
            .nth(index)
            .ok_or_else(|| anyhow::anyhow!("Audio device at index {} not found", index))?
    } else {
        host.default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No default audio output device available"))?
    };

    println!("Using audio device: \"{}\"", device.name().unwrap_or_else(|_| "Unknown".to_string()));

    let config = device.default_output_config()?;
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;

    println!(
        "Audio Setup: freq={}Hz, channels={}, voices={}",
        sample_rate, channels, num_voices
    );

    let mut two_op_synth = TwoOpSynth::new(sample_rate, num_voices);

    let err_fn = |err| eprintln!("An error occurred on the audio stream: {}", err);

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            AudioMessage::TwoOp(two_op_msg) => match two_op_msg {
                                TwoOpMessage::NoteOn { freq, vel } => two_op_synth.note_on(freq, vel),
                                TwoOpMessage::NoteOff { freq } => two_op_synth.note_off(freq),
                                TwoOpMessage::NoteOffAll => two_op_synth.note_off_all(),
                                TwoOpMessage::TimedNote { freq, vel, duration_ms } => two_op_synth.trigger_note(freq, vel, duration_ms),
                                TwoOpMessage::SetLegato { enabled } => two_op_synth.set_legato(enabled),
                                TwoOpMessage::SetRealtimeUpdate { enabled } => two_op_synth.set_realtime_update(enabled),
                                TwoOpMessage::SetMode { mode } => two_op_synth.set_mode(mode),
                                TwoOpMessage::SetOsc1Waveform { waveform } => two_op_synth.set_osc1_waveform(waveform),
                                TwoOpMessage::SetOsc2Waveform { waveform } => two_op_synth.set_osc2_waveform(waveform),
                                TwoOpMessage::SetOsc1Adsr { a, d, s, r } => two_op_synth.set_osc1_adsr(a, d, s, r),
                                TwoOpMessage::SetOsc2Adsr { a, d, s, r } => two_op_synth.set_osc2_adsr(a, d, s, r),
                                TwoOpMessage::SetFilterAdsr { a, d, s, r } => two_op_synth.set_filter_adsr(a, d, s, r),
                                TwoOpMessage::SetFilterParams { cutoff, q, filter_type } => two_op_synth.set_filter_params(cutoff, q, filter_type),
                                TwoOpMessage::SetFilterMod { enabled, amount } => two_op_synth.set_filter_mod(enabled, amount),
                                TwoOpMessage::SetModulationParams { index, ratio, detune } => two_op_synth.set_modulation_params(index, ratio, detune),
                            },
                        }
                    }

                    let sample = two_op_synth.process();
                    for channel in frame.iter_mut() {
                        *channel = sample;
                    }
                }
            },
            err_fn,
            None,
        )?,
        sample_format => return Err(anyhow::anyhow!("Unsupported sample format: {}", sample_format)),
    };

    stream.play()?;
    Ok(stream)
}
