use aillen_core::dsp::{filter::FilterType, oscillator::Waveform, ModulationSource, DelayMode, distortion::DistortionMode};
use aillen_core::synth::two_op::SynthMode;
use aillen_core::synth::sampler::{PlayMode, load_audio_file, StretchMode};
use aillen_core::mixer::Mixer;
use aillen_core::synth::two_op::two_op::TwoOpSynth;
use aillen_core::synth::sampler::Sampler;
use aillen_core::synth::synth303::synth303::Synth303;
use aillen_core::synth::hubass::hubass::SynthHubass;
use anyhow::Result;
use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Receiver;
use std::sync::Arc;

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
    /// Sets Legato mode on the TwoOp synth.
    TwoOpSetLegato { 
        track_id: usize,
        /// Legato toggle state.
        enabled: bool 
    },
    /// Sets real-time parameter updating on active notes for the TwoOp synth.
    TwoOpSetRealtimeUpdate { 
        track_id: usize,
        /// Toggle state.
        enabled: bool 
    },
    /// Sets the active synthesis mode for the TwoOp synth.
    TwoOpSetMode { 
        track_id: usize,
        /// Synthesis algorithm.
        mode: SynthMode 
    },
    /// Sets the waveform of Operator 1 (Carrier) for the TwoOp synth.
    TwoOpSetOsc1Waveform { 
        track_id: usize,
        /// Target waveform.
        waveform: Waveform 
    },
    /// Sets the waveform of Operator 2 (Modulator) for the TwoOp synth.
    TwoOpSetOsc2Waveform { 
        track_id: usize,
        /// Target waveform.
        waveform: Waveform 
    },
    /// Sets Operator 1 ADSR envelope parameters.
    TwoOpSetOsc1Adsr { 
        track_id: usize,
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
        track_id: usize,
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
        track_id: usize,
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
        track_id: usize,
        /// Base cutoff frequency in Hz.
        cutoff: f32, 
        /// Filter resonance Q-factor.
        q: f32, 
        /// Biquad filter type.
        filter_type: FilterType 
    },
    /// Enables/disables filter cutoff envelope modulation and sets its depth.
    TwoOpSetFilterMod { 
        track_id: usize,
        /// Toggle state.
        enabled: bool, 
        /// Modulation depth in Hz.
        amount: f32 
    },
    /// Sets modulator synthesis properties.
    TwoOpSetModulationParams { 
        track_id: usize,
        /// Modulation index.
        index: f32, 
        /// Modulator frequency ratio relative to Carrier.
        ratio: f32, 
        /// Modulator detuning in Hz.
        detune: f32 
    },
    /// Sets Operator 2 self-feedback.
    TwoOpSetOsc2Feedback {
        track_id: usize,
        feedback: f32,
    },
    /// Sets Operator 2 wavefolder.
    TwoOpSetWavefold {
        track_id: usize,
        gain: f32,
        mix: f32,
    },
    /// Sets phase noise injection.
    TwoOpSetNoise {
        track_id: usize,
        carrier_noise: f32,
        modulator_noise: f32,
    },
    /// Sets pitch sweep envelope parameters.
    TwoOpSetPitchSweep {
        track_id: usize,
        depth: f32,
        decay: f32,
    },
    /// Sets voice LFO properties.
    TwoOpSetLfo {
        track_id: usize,
        waveform: usize,
        speed: f32,
        mod_index: f32,
        cutoff: f32,
    },

    // Sampler specific
    /// Loads an audio file into the Sampler buffer.
    SamplerLoadSample { 
        track_id: usize,
        /// Path to the audio file on disk.
        path: String 
    },
    /// Sets the Sampler playback mode.
    SamplerSetPlayMode { 
        track_id: usize,
        /// Playback mode (OneShot or Loop).
        mode: PlayMode 
    },
    /// Sets the Sampler pitch ratio factor.
    SamplerSetPitchRatio { 
        track_id: usize,
        /// Pitch scaling multiplier.
        ratio: f32 
    },
    /// Sets the Sampler playback speed ratio factor.
    SamplerSetSpeedRatio { 
        track_id: usize,
        /// Speed scaling multiplier.
        ratio: f32 
    },
    /// Sets the Sampler time-stretching engine mode.
    SamplerSetStretchMode { 
        track_id: usize,
        /// Decoupled granular or linked resampler mode.
        mode: StretchMode 
    },
    /// Sets Sampler grain duration size in milliseconds.
    SamplerSetGrainSize { 
        track_id: usize,
        /// Grain duration.
        size_ms: f32 
    },
    /// Sets Sampler overlapping grains count.
    SamplerSetOverlap { 
        track_id: usize,
        /// Overlapping grains.
        overlap: usize 
    },
    /// Loads a preloaded sample buffer directly into the Sampler buffer.
    SamplerLoadBuffer {
        track_id: usize,
        /// Shared sample buffer wrapper.
        buffer: Arc<aillen_core::synth::sampler::SampleBuffer>,
    },

    // Mixer settings
    /// Sets the volume gain of a specific track.
    SetTrackVolume { 
        /// Track index.
        track_id: usize, 
        /// Volume gain.
        volume: f32 
    },
    /// Sets the sidechain source of a specific track.
    SetTrackSidechainSource {
        /// Track index.
        track_id: usize,
        /// Optional source track index.
        source_id: Option<usize>,
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
    /// Sets the Sampler DJ performance filter position.
    SamplerSetDjFilter {
        track_id: usize,
        /// Position from -1.0 to 1.0.
        position: f32
    },
    /// Enables/disables slice playback mode.
    SamplerSetSliceMode {
        track_id: usize,
        /// Toggle state.
        enabled: bool
    },
    /// Sets the total number of slices.
    SamplerSetNumSlices {
        track_id: usize,
        /// Number of slices.
        n: usize
    },
    /// Selects the active slice index.
    SamplerSetSelectedSlice {
        track_id: usize,
        /// Slice index.
        slice: usize
    },
    /// Sets the stutter repetitions count.
    SamplerSetStutterCount {
        track_id: usize,
        /// Repeat count.
        count: usize
    },
    /// Sets the Mixer master output DJ performance filter position.
    MixerSetMasterFilter {
        /// Position from -1.0 to 1.0.
        position: f32
    },
    
    // Synth303 specific (Track 6)
    Synth303SetWaveform { waveform: Waveform },
    Synth303SetAmpAdsr { a: f32, d: f32, s: f32, r: f32 },
    Synth303SetFilterAdsr { a: f32, d: f32, s: f32, r: f32 },
    Synth303SetPitchAdsr { a: f32, d: f32, s: f32, r: f32 },
    Synth303SetFilterParams { cutoff: f32, resonance: f32 },
    Synth303SetFilterMod { amount: f32 },
    Synth303SetPitchMod { amount: f32 },
    Synth303SetPwmParams { pw: f32, rate: f32, depth: f32 },
    Synth303SetGlideTime { seconds: f32 },
    Synth303SetLegato { enabled: bool },

    // SynthHubass specific (Track 7)
    SynthHubassSetAmpAdsr { a: f32, d: f32, s: f32, r: f32 },
    SynthHubassSetFilterParams { start_mult: f32, end_cf: f32, decay: f32, resonance: f32 },
    SynthHubassSetOscUnison { waveform: i32, detune: f32, spread: f32, num_voices: i32 },
    SynthHubassSetOscSub { waveform: i32, octave_offset: i32, gain: f32 },
    SynthHubassSetOscNoise { gain: f32 },
    SynthHubassSetFilterMode { mode: i32 },
    SynthHubassSetDriveMode { mode: i32, gain: f32, mix: f32 },
    SynthHubassSetLfo1 { waveform: i32, speed_hz: f32, cutoff_depth: f32, pitch_depth: f32 },
    SynthHubassSetChorusParams { mix: f32, depth: f32 },
    SynthHubassSetLegato { enabled: bool },
    SynthHubassSetOutputGain { gain: f32 },

    // FxChain & Return Track settings
    SetTrackSendDelay { track_id: usize, send: f32 },
    SetTrackRmDepth { track_id: usize, depth: f32 },
    SetTrackRmFreq { track_id: usize, freq: f32 },
    SetTrackRmMode { track_id: usize, ring_mod: bool },
    SetTrackRmSource { track_id: usize, source: usize },
    SetTrackFxFilterPos { track_id: usize, pos: f32 },
    SetTrackCompThreshold { track_id: usize, thresh: f32 },
    SetTrackCompRatio { track_id: usize, ratio: f32 },
    SetTrackCompAttack { track_id: usize, attack: f32 },
    SetTrackCompRelease { track_id: usize, release: f32 },
    SetTrackCompMakeup { track_id: usize, makeup: f32 },
    SetReturnDelayTime { time: f32 },
    SetReturnDelayFeedback { feedback: f32 },
    SetReturnDelayMode { mode: usize },
    SetReturnDelayPingPong { enabled: bool },
    SetReturnDelayDrive { drive: f32 },
    SetReturnDelayGrainSize { size: f32 },
    SetReturnDelayDensity { density: usize },
    SetReturnDelaySpray { spray: f32 },
    SetReturnDelayPitch { pitch: f32 },
    SetTrackCompSidechain { track_id: usize, enabled: bool },
    SetTrackDistortionMode { track_id: usize, mode: i32 },
    SetTrackDistortionDrive { track_id: usize, drive: f32 },
    SetTrackDistortionMix { track_id: usize, mix: f32 },
    SetMasterLimiterGain { gain: f32 },
    SetMasterLimiterRelease { release: f32 },
    SetMasterLimiterCeiling { ceiling: f32 },
    SetMasterWlDrop { drop: usize },
    SetMasterWlOutof { outof: usize },
    SetMasterWlMode { mode: usize },
    GlobalPanic,
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
pub fn start_audio_thread(
    rx: Receiver<AudioMessage>,
    num_voices: usize,
    device_index: Option<usize>,
    ui_handle: crate::ui::UiHandle,
) -> Result<cpal::Stream> {
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
                                     if let Some(two_op) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                         two_op.trigger_note(freq, vel, duration_ms);
                                     } else if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                         sampler.note_on(freq, vel);
                                     } else if let Some(synth303) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Synth303>() {
                                         synth303.trigger_note(freq, vel, duration_ms);
                                     } else if let Some(hubass) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                         hubass.trigger_note(freq, vel, duration_ms);
                                     }
                                 }
                             }
                              AudioMessage::TwoOpSetLegato { track_id, enabled } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_legato(enabled);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetRealtimeUpdate { track_id, enabled } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_realtime_update(enabled);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetMode { track_id, mode } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_mode(mode);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetOsc1Waveform { track_id, waveform } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_osc1_waveform(waveform);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetOsc2Waveform { track_id, waveform } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_osc2_waveform(waveform);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetOsc1Adsr { track_id, a, d, s, r } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_osc1_adsr(a, d, s, r);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetOsc2Adsr { track_id, a, d, s, r } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_osc2_adsr(a, d, s, r);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetFilterAdsr { track_id, a, d, s, r } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_filter_adsr(a, d, s, r);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetFilterParams { track_id, cutoff, q, filter_type } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_filter_params(cutoff, q, filter_type);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetFilterMod { track_id, enabled, amount } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_filter_mod(enabled, amount);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetModulationParams { track_id, index, ratio, detune } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_modulation_params(index, ratio, detune);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetOsc2Feedback { track_id, feedback } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_osc2_feedback(feedback);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetWavefold { track_id, gain, mix } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_wavefold(gain, mix);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetNoise { track_id, carrier_noise, modulator_noise } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_noise(carrier_noise, modulator_noise);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetPitchSweep { track_id, depth, decay } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_pitch_sweep(depth, decay);
                                      }
                                  }
                              }
                              AudioMessage::TwoOpSetLfo { track_id, waveform, speed, mod_index, cutoff } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(synth) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<TwoOpSynth>() {
                                          synth.set_lfo(waveform, speed, mod_index, cutoff);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetWaveform { waveform } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_waveform(waveform);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetAmpAdsr { a, d, s, r } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_amp_adsr(a, d, s, r);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetFilterAdsr { a, d, s, r } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_filter_adsr(a, d, s, r);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetPitchAdsr { a, d, s, r } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_pitch_adsr(a, d, s, r);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetFilterParams { cutoff, resonance } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_filter_params(cutoff, resonance);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetFilterMod { amount } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_filter_mod(amount);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetPitchMod { amount } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_pitch_mod(amount);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetPwmParams { pw, rate, depth } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_pwm_params(pw, rate, depth);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetGlideTime { seconds } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_glide_time(seconds);
                                      }
                                  }
                              }
                              AudioMessage::Synth303SetLegato { enabled } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<Synth303>() {
                                          synth.set_legato(enabled);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetAmpAdsr { a, d, s, r } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_amp_adsr(a, d, s, r);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetFilterParams { start_mult, end_cf, decay, resonance } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_filter_params(start_mult, end_cf, decay, resonance);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetOscUnison { waveform, detune, spread, num_voices } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_osc_unison(waveform, detune, spread, num_voices);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetOscSub { waveform, octave_offset, gain } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_osc_sub(waveform, octave_offset, gain);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetOscNoise { gain } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_osc_noise(gain);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetFilterMode { mode } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_filter_mode(mode);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetDriveMode { mode, gain, mix } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_drive_mode(mode, gain, mix);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetLfo1 { waveform, speed_hz, cutoff_depth, pitch_depth } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_lfo1(waveform, speed_hz, cutoff_depth, pitch_depth);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetChorusParams { mix, depth } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_chorus_params(mix, depth);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetLegato { enabled } => {
                                  for track in &mut mixer.tracks {
                                      if let Some(synth) = track.instrument.as_any_mut().downcast_mut::<SynthHubass>() {
                                          synth.set_legato(enabled);
                                      }
                                  }
                              }
                              AudioMessage::SynthHubassSetOutputGain { gain } => {
                                  for track in &mut mixer.tracks {
                                          synth.set_output_gain(gain);
                                      }
                                  }
                              }
                              AudioMessage::SamplerLoadSample { track_id, path } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          println!("Loading sample on track {} from path: {}", track_id, path);
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
                              }
                              AudioMessage::SamplerSetPlayMode { track_id, mode } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_play_mode(mode);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetPitchRatio { track_id, ratio } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_pitch_ratio(ratio);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetSpeedRatio { track_id, ratio } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_speed_ratio(ratio);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetStretchMode { track_id, mode } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_stretch_mode(mode);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetGrainSize { track_id, size_ms } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_grain_size(size_ms);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetOverlap { track_id, overlap } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_overlap(overlap);
                                      }
                                  }
                              }
                              AudioMessage::SamplerLoadBuffer { track_id, buffer } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.sample_buffer = Some(buffer.clone());
                                          for voice in &mut sampler.voices {
                                              voice.sample_buffer = Some(buffer.clone());
                                          }
                                          println!("Sampler on track {}: Switched to preloaded buffer from SampleBank!", track_id);
                                      }
                                  }
                              }
                              AudioMessage::SetTrackVolume { track_id, volume } => {
                                  if track_id < mixer.tracks.len() {
                                      mixer.tracks[track_id].set_volume(volume);
                                  }
                              }
                              AudioMessage::SetTrackSidechainSource { track_id, source_id } => {
                                  if track_id < mixer.tracks.len() {
                                      mixer.tracks[track_id].sidechain_source = source_id;
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
                              AudioMessage::SamplerSetDjFilter { track_id, position } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_dj_filter_position(position);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetSliceMode { track_id, enabled } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_slice_mode(enabled);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetNumSlices { track_id, n } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_num_slices(n);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetSelectedSlice { track_id, slice } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_selected_slice(slice);
                                      }
                                  }
                              }
                              AudioMessage::SamplerSetStutterCount { track_id, count } => {
                                  if track_id < mixer.tracks.len() {
                                      if let Some(sampler) = mixer.tracks[track_id].instrument.as_any_mut().downcast_mut::<Sampler>() {
                                          sampler.set_stutter_count(count);
                                      }
                                  }
                              }
                            AudioMessage::MixerSetMasterFilter { position } => {
                                mixer.set_master_filter_position(position);
                            }
                            AudioMessage::SetTrackSendDelay { track_id, send } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].send_delay = send;
                                }
                            }
                            AudioMessage::SetTrackRmDepth { track_id, depth } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.ring_mod_l.depth = depth;
                                    mixer.tracks[track_id].fx_chain.ring_mod_r.depth = depth;
                                }
                            }
                            AudioMessage::SetTrackRmFreq { track_id, freq } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.ring_mod_l.frequency = freq;
                                    mixer.tracks[track_id].fx_chain.ring_mod_r.frequency = freq;
                                }
                            }
                            AudioMessage::SetTrackRmMode { track_id, ring_mod } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.ring_mod_l.ring_mod = ring_mod;
                                    mixer.tracks[track_id].fx_chain.ring_mod_r.ring_mod = ring_mod;
                                }
                            }
                            AudioMessage::SetTrackRmSource { track_id, source } => {
                                if track_id < mixer.tracks.len() {
                                    let src = match source {
                                        1 => ModulationSource::SelfMod,
                                        2 => ModulationSource::Sidechain,
                                        _ => ModulationSource::Sine,
                                    };
                                    mixer.tracks[track_id].fx_chain.ring_mod_l.source = src;
                                    mixer.tracks[track_id].fx_chain.ring_mod_r.source = src;
                                }
                            }
                            AudioMessage::SetTrackFxFilterPos { track_id, pos } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.dj_filter_l.set_position(pos);
                                    mixer.tracks[track_id].fx_chain.dj_filter_r.set_position(pos);
                                }
                            }
                            AudioMessage::SetTrackCompThreshold { track_id, thresh } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.compressor_l.threshold = thresh;
                                    mixer.tracks[track_id].fx_chain.compressor_r.threshold = thresh;
                                }
                            }
                            AudioMessage::SetTrackCompRatio { track_id, ratio } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.compressor_l.ratio = ratio;
                                    mixer.tracks[track_id].fx_chain.compressor_r.ratio = ratio;
                                }
                            }
                            AudioMessage::SetTrackCompAttack { track_id, attack } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.compressor_l.attack = attack;
                                    mixer.tracks[track_id].fx_chain.compressor_r.attack = attack;
                                }
                            }
                            AudioMessage::SetTrackCompRelease { track_id, release } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.compressor_l.release = release;
                                    mixer.tracks[track_id].fx_chain.compressor_r.release = release;
                                }
                            }
                            AudioMessage::SetTrackCompMakeup { track_id, makeup } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.compressor_l.makeup_gain = makeup;
                                    mixer.tracks[track_id].fx_chain.compressor_r.makeup_gain = makeup;
                                }
                            }
                            AudioMessage::SetReturnDelayTime { time } => {
                                mixer.return_delay.tape.delay_time = time;
                                mixer.return_delay.granular.delay_time = time;
                            }
                            AudioMessage::SetReturnDelayFeedback { feedback } => {
                                mixer.return_delay.tape.feedback = feedback;
                                mixer.return_delay.granular.feedback = feedback;
                            }
                            AudioMessage::SetReturnDelayMode { mode } => {
                                let d_mode = match mode {
                                    1 => DelayMode::Granular,
                                    _ => DelayMode::Tape,
                                };
                                mixer.return_delay.mode = d_mode;
                            }
                            AudioMessage::SetReturnDelayPingPong { enabled } => {
                                mixer.return_delay.tape.ping_pong = enabled;
                            }
                            AudioMessage::SetReturnDelayDrive { drive } => {
                                mixer.return_delay.tape.drive = drive;
                            }
                            AudioMessage::SetReturnDelayGrainSize { size } => {
                                mixer.return_delay.granular.grain_size = size;
                            }
                            AudioMessage::SetReturnDelayDensity { density } => {
                                mixer.return_delay.granular.density = density;
                            }
                            AudioMessage::SetReturnDelaySpray { spray } => {
                                mixer.return_delay.granular.spray = spray;
                            }
                            AudioMessage::SetReturnDelayPitch { pitch } => {
                                mixer.return_delay.granular.pitch = pitch;
                            }
                            AudioMessage::SetTrackCompSidechain { track_id, enabled } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.compressor_sidechain = enabled;
                                }
                            }
                            AudioMessage::SetTrackDistortionMode { track_id, mode } => {
                                if track_id < mixer.tracks.len() {
                                    let mode_enum = DistortionMode::from_i32(mode);
                                    mixer.tracks[track_id].fx_chain.distortion_l.mode = mode_enum;
                                    mixer.tracks[track_id].fx_chain.distortion_r.mode = mode_enum;
                                }
                            }
                            AudioMessage::SetTrackDistortionDrive { track_id, drive } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.distortion_l.drive = drive;
                                    mixer.tracks[track_id].fx_chain.distortion_r.drive = drive;
                                }
                            }
                            AudioMessage::SetTrackDistortionMix { track_id, mix } => {
                                if track_id < mixer.tracks.len() {
                                    mixer.tracks[track_id].fx_chain.distortion_l.mix = mix;
                                    mixer.tracks[track_id].fx_chain.distortion_r.mix = mix;
                                }
                            }
                            AudioMessage::SetMasterLimiterGain { gain } => {
                                mixer.master_limiter.threshold_gain = gain;
                            }
                            AudioMessage::SetMasterLimiterRelease { release } => {
                                mixer.master_limiter.release_s = release;
                            }
                            AudioMessage::SetMasterLimiterCeiling { ceiling } => {
                                mixer.master_limiter.ceiling = ceiling;
                            }
                            AudioMessage::SetMasterWlDrop { drop } => {
                                mixer.master_waveloss_l.drop = drop;
                                mixer.master_waveloss_r.drop = drop;
                            }
                            AudioMessage::SetMasterWlOutof { outof } => {
                                mixer.master_waveloss_l.outof = outof;
                                mixer.master_waveloss_r.outof = outof;
                            }
                            AudioMessage::SetMasterWlMode { mode } => {
                                mixer.master_waveloss_l.mode = mode;
                                mixer.master_waveloss_r.mode = mode;
                            }
                            AudioMessage::GlobalPanic => {
                                for track in &mut mixer.tracks {
                                    track.instrument.note_off_all();
                                }
                            }
                        }
                    }

                    let (track_outs, (sample_l, sample_r)) = mixer.process_detailed();
                    ui_handle.record_audio_frame(&track_outs, sample_l, sample_r);
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
