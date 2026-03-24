use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use aillen_core::dsp::AudioNode;
use aillen_core::synth::poly::PolySynth;
use crossbeam_channel::Receiver;
use anyhow::Result;

pub enum AudioMessage {
    NoteOn { freq: f32, vel: f32 },
    NoteOff { freq: f32 },
    NoteOffAll,
    TimedNote { freq: f32, vel: f32, duration_ms: f32 },
    SetLegato { enabled: bool },
}

pub fn start_audio_thread(rx: Receiver<AudioMessage>, num_voices: usize) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No default audio output device available");
    
    let config = device.default_output_config()?;
    let sample_rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    
    println!("Audio Setup: freq={}Hz, channels={}, voices={}", sample_rate, channels, num_voices);
    
    let mut synth = PolySynth::new(sample_rate, num_voices);
    
    let err_fn = |err| eprintln!("An error occurred on the audio stream: {}", err);
    
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            AudioMessage::NoteOn { freq, vel } => synth.note_on(freq, vel),
                            AudioMessage::NoteOff { freq } => synth.note_off(freq),
                            AudioMessage::NoteOffAll => synth.note_off_all(),
                            AudioMessage::TimedNote { freq, vel, duration_ms } => synth.trigger_note(freq, vel, duration_ms),
                            AudioMessage::SetLegato { enabled } => synth.set_legato(enabled),
                        }
                    }
                    
                    let sample = synth.process();
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
