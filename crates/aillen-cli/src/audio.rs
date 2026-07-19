use aillen_core::dsp::{filter::FilterType, oscillator::Waveform};
use aillen_core::synth::two_op::SynthMode;
use aillen_core::synth::sampler::{PlayMode, load_audio_file, StretchMode};
use aillen_core::mixer::{Mixer, Instrument};
use anyhow::Result;
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Receiver;

/// Message types sent from the OSC server thread to the real-time audio thread.
pub enum AudioMessage {
    /// Triggers note start for a specific track.
    TrackNoteOn { 
        /// Channel index in the Mixer.
        track_id: usize, 
        /// Frequency in Hz.
        freq: f32, 
        /// Note velocity gain (0.0 to 1.0).
        vel: f32 
    },
    /// Triggers note release for a specific track.
    TrackNoteOff { 
        /// Channel index in the Mixer.
        track_id: usize, 
        /// Frequency in Hz.
        freq: f32 
    },
    /// Silences all active notes on a specific track immediately.
    TrackNoteOffAll { 
        /// Channel index in the Mixer.
        track_id: usize 
    },
    /// Plays a timed note (triggers on, then silences after duration_ms).
    TrackTimedNote { 
        /// Channel index in the Mixer.
        track_id: usize, 
        /// Frequency in Hz.
        freq: f32, 
        /// Note velocity gain.
        vel: f32, 
        /// Duration of the note in milliseconds.
        duration_ms: f32 
    },
    
    // TwoOp specific
    /// Sets Legato mode on the TwoOp synth (Track 0).
    TwoOpSetLegato { 
        /// Legato toggle state.
        enabled: bool 
    },
    /// Sets real-time parameter updating on active notes for the TwoOp synth.
    TwoOpSetRealtimeUpdate { 
        /// Toggle state.
        enabled: bool 
    },
    /// Sets the active synthesis mode for the TwoOp synth.
    TwoOpSetMode { 
        /// Synthesis algorithm.
        mode: SynthMode 
    },
    /// Sets the waveform of Operator 1 (Carrier) for the TwoOp synth.
    TwoOpSetOsc1Waveform { 
        /// Target waveform.
        waveform: Waveform 
    },
    /// Sets the waveform of Operator 2 (Modulator) for the TwoOp synth.
    TwoOpSetOsc2Waveform { 
        /// Target waveform.
        waveform: Waveform 
    },
    /// Sets Operator 1 ADSR envelope parameters.
    TwoOpSetOsc1Adsr { 
        /// Attack time in seconds.
        a: f32, 
        /// Decay time in seconds.
        d: f32, 
        /// Sustain level amplitude.
        s: f32, 
        /// Release time in seconds.
        r: f32 
    },
    /// Sets Operator 2 ADSR envelope parameters.
    TwoOpSetOsc2Adsr { 
        /// Attack time in seconds.
        a: f32, 
        /// Decay time in seconds.
        d: f32, 
        /// Sustain level amplitude.
        s: f32, 
        /// Release time in seconds.
        r: f32 
    },
    /// Sets Filter Cutoff ADSR envelope parameters.
    TwoOpSetFilterAdsr { 
        /// Attack time in seconds.
        a: f32, 
        /// Decay time in seconds.
        d: f32, 
        /// Sustain level amplitude.
        s: f32, 
        /// Release time in seconds.
        r: f32 
    },
    /// Sets biquad filter properties.
    TwoOpSetFilterParams { 
        /// Base cutoff frequency in Hz.
        cutoff: f32, 
        /// Filter resonance Q-factor.
        q: f32, 
        /// Biquad filter type.
        filter_type: FilterType 
    },
    /// Enables/disables filter cutoff envelope modulation and sets its depth.
    TwoOpSetFilterMod { 
        /// Toggle state.
        enabled: bool, 
        /// Modulation depth in Hz.
        amount: f32 
    },
    /// Sets modulator synthesis properties.
    TwoOpSetModulationParams { 
        /// Modulation index.
        index: f32, 
        /// Modulator frequency ratio relative to Carrier.
        ratio: f32, 
        /// Modulator detuning in Hz.
        detune: f32 
    },

    // Sampler specific
    /// Loads an audio file into the Sampler (Track 1) buffer.
    SamplerLoadSample { 
        /// Path to the audio file on disk.
        path: String 
    },
    /// Sets the Sampler playback mode.
    SamplerSetPlayMode { 
        /// Playback mode (OneShot or Loop).
        mode: PlayMode 
    },
    /// Sets the Sampler pitch ratio factor.
    SamplerSetPitchRatio { 
        /// Pitch scaling multiplier.
        ratio: f32 
    },
    /// Sets the Sampler playback speed ratio factor.
    SamplerSetSpeedRatio { 
        /// Speed scaling multiplier.
        ratio: f32 
    },
    /// Sets the Sampler time-stretching engine mode.
    SamplerSetStretchMode { 
        /// Decoupled granular or linked resampler mode.
        mode: StretchMode 
    },
    /// Sets Sampler grain duration size in milliseconds.
    SamplerSetGrainSize { 
        /// Grain duration.
        size_ms: f32 
    },
    /// Sets Sampler overlapping grains count.
    SamplerSetOverlap { 
        /// Overlapping grains.
        overlap: usize 
    },

    // Mixer settings
    /// Sets the volume gain of a specific track.
    SetTrackVolume { 
        /// Track index.
        track_id: usize, 
        /// Volume gain.
        volume: f32 
    },
    /// Sets the panning of a specific track.
    SetTrackPan { 
        /// Track index.
        track_id: usize, 
        /// Panning value from -1.0 to 1.0.
        pan: f32 
    },
    /// Mutes or unmutes a specific track.
    SetTrackMute { 
        /// Track index.
        track_id: usize, 
        /// Mute state.
        mute: bool 
    },
    /// Sets the global master output volume gain.
    SetMasterVolume { 
        /// Master volume gain.
        volume: f32 
    },
}

/// Lists all available host audio output devices and their indices.
pub fn list_audio_devices() -> Result<()> {
    let host = cpal::default_host();
    let devices = host.output_devices()?;
    println!("Available Audio Devices:");
    for (i, device) in devices.enumerate() {
        println!("{}. {}", i, device.name().unwrap_or_else(|_| "Unknown".to_string()));
    }
    Ok(())
}

/// Spawns the real-time audio thread, setting up CPAL output stream and routing messages.
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
                            AudioMessage::SamplerSetStretchMode { mode } => {
                                if let Instrument::Sampler(sampler) = &mut mixer.tracks[1].instrument {
                                    sampler.set_stretch_mode(mode);
                                }
                            }
                            AudioMessage::SamplerSetGrainSize { size_ms } => {
                                if let Instrument::Sampler(sampler) = &mut mixer.tracks[1].instrument {
                                    sampler.set_grain_size(size_ms);
                                }
                            }
                            AudioMessage::SamplerSetOverlap { overlap } => {
                                if let Instrument::Sampler(sampler) = &mut mixer.tracks[1].instrument {
                                    sampler.set_overlap(overlap);
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
