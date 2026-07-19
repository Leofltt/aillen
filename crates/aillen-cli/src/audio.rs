use aillen_core::dsp::{filter::FilterType, oscillator::Waveform};
use aillen_core::synth::two_op::SynthMode;
use aillen_core::synth::sampler::{PlayMode, load_audio_file};
use aillen_core::mixer::{Mixer, Instrument};
use anyhow::Result;
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Receiver;

pub enum AudioMessage {
    TrackNoteOn { track_id: usize, freq: f32, vel: f32 },
    TrackNoteOff { track_id: usize, freq: f32 },
    TrackNoteOffAll { track_id: usize },
    TrackTimedNote { track_id: usize, freq: f32, vel: f32, duration_ms: f32 },
    
    // TwoOp specific
    TwoOpSetLegato { enabled: bool },
    TwoOpSetRealtimeUpdate { enabled: bool },
    TwoOpSetMode { mode: SynthMode },
    TwoOpSetOsc1Waveform { waveform: Waveform },
    TwoOpSetOsc2Waveform { waveform: Waveform },
    TwoOpSetOsc1Adsr { a: f32, d: f32, s: f32, r: f32 },
    TwoOpSetOsc2Adsr { a: f32, d: f32, s: f32, r: f32 },
    TwoOpSetFilterAdsr { a: f32, d: f32, s: f32, r: f32 },
    TwoOpSetFilterParams { cutoff: f32, q: f32, filter_type: FilterType },
    TwoOpSetFilterMod { enabled: bool, amount: f32 },
    TwoOpSetModulationParams { index: f32, ratio: f32, detune: f32 },

    // Sampler specific
    SamplerLoadSample { path: String },
    SamplerSetPlayMode { mode: PlayMode },
    SamplerSetPitchRatio { ratio: f32 },
    SamplerSetSpeedRatio { ratio: f32 },

    // Mixer settings
    SetTrackVolume { track_id: usize, volume: f32 },
    SetTrackPan { track_id: usize, pan: f32 },
    SetTrackMute { track_id: usize, mute: bool },
    SetMasterVolume { volume: f32 },
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

    let mut mixer = Mixer::new(sample_rate, num_voices);

    let err_fn = |err| eprintln!("An error occurred on the audio stream: {}", err);

    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for frame in data.chunks_mut(channels) {
                    while let Ok(msg) = rx.try_recv() {
                        match msg {
                            AudioMessage::TrackNoteOn { track_id, freq, vel } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].instrument.note_on(freq, vel);
                                }
                            }
                            AudioMessage::TrackNoteOff { track_id, freq } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].instrument.note_off(freq);
                                }
                            }
                            AudioMessage::TrackNoteOffAll { track_id } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].instrument.note_off_all();
                                }
                            }
                            AudioMessage::TrackTimedNote { track_id, freq, vel, duration_ms } => {
                                if track_id < mixer.tracks.len() {
                                    match &mut mixer.tracks[track_id].instrument {
                                        Instrument::TwoOp(two_op) => {
                                            two_op.trigger_note(freq, vel, duration_ms);
                                        }
                                        Instrument::Sampler(sampler) => {
                                            sampler.note_on(freq, vel);
                                            // Trigger note off later? (A simple timeout is currently handled by the synth voice,
                                            // for Sampler a TimedNote is currently mapped to standard NoteOn).
                                        }
                                    }
                                }
                            }
                            AudioMessage::TwoOpSetLegato { enabled } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_legato(enabled);
                                }
                            }
                            AudioMessage::TwoOpSetRealtimeUpdate { enabled } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_realtime_update(enabled);
                                }
                            }
                            AudioMessage::TwoOpSetMode { mode } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_mode(mode);
                                }
                            }
                            AudioMessage::TwoOpSetOsc1Waveform { waveform } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_osc1_waveform(waveform);
                                }
                            }
                            AudioMessage::TwoOpSetOsc2Waveform { waveform } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_osc2_waveform(waveform);
                                }
                            }
                            AudioMessage::TwoOpSetOsc1Adsr { a, d, s, r } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_osc1_adsr(a, d, s, r);
                                }
                            }
                            AudioMessage::TwoOpSetOsc2Adsr { a, d, s, r } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_osc2_adsr(a, d, s, r);
                                }
                            }
                            AudioMessage::TwoOpSetFilterAdsr { a, d, s, r } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_filter_adsr(a, d, s, r);
                                }
                            }
                            AudioMessage::TwoOpSetFilterParams { cutoff, q, filter_type } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_filter_params(cutoff, q, filter_type);
                                }
                            }
                            AudioMessage::TwoOpSetFilterMod { enabled, amount } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_filter_mod(enabled, amount);
                                }
                            }
                            AudioMessage::TwoOpSetModulationParams { index, ratio, detune } => {
                                if let Instrument::TwoOp(synth) = &mut mixer.tracks[0].instrument {
                                    synth.set_modulation_params(index, ratio, detune);
                                }
                            }
                            AudioMessage::SamplerLoadSample { path } => {
                                if let Instrument::Sampler(sampler) = &mut mixer.tracks[1].instrument {
                                    println!("Loading sample from path: {}", path);
                                    match load_audio_file(&path) {
                                        Ok(buf) => {
                                            sampler.set_sample(buf);
                                            println!("Sample loaded successfully!");
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to load sample file: {:?}", e);
                                        }
                                    }
                                }
                            }
                            AudioMessage::SamplerSetPlayMode { mode } => {
                                if let Instrument::Sampler(sampler) = &mut mixer.tracks[1].instrument {
                                    sampler.set_play_mode(mode);
                                }
                            }
                            AudioMessage::SamplerSetPitchRatio { ratio } => {
                                if let Instrument::Sampler(sampler) = &mut mixer.tracks[1].instrument {
                                    sampler.set_pitch_ratio(ratio);
                                }
                            }
                            AudioMessage::SamplerSetSpeedRatio { ratio } => {
                                if let Instrument::Sampler(sampler) = &mut mixer.tracks[1].instrument {
                                    sampler.set_speed_ratio(ratio);
                                }
                            }
                            AudioMessage::SetTrackVolume { track_id, volume } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].set_volume(volume);
                                }
                            }
                            AudioMessage::SetTrackPan { track_id, pan } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].set_pan(pan);
                                }
                            }
                            AudioMessage::SetTrackMute { track_id, mute } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].set_mute(mute);
                                }
                            }
                            AudioMessage::SetMasterVolume { volume } => {
                                mixer.set_master_volume(volume);
                            }
                        }
                    }

                    let (sample_l, sample_r) = mixer.process();
                    if channels >= 2 {
                        frame[0] = sample_l;
                        frame[1] = sample_r;
                        for c in 2..channels {
                            frame[c] = 0.0;
                        }
                    } else if channels == 1 {
                        frame[0] = (sample_l + sample_r) * 0.5;
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
