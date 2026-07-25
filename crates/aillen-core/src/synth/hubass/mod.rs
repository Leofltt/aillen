use crate::dsp::{
    oscillator::{Waveform, SubWaveform},
    lfo::LfoWaveform,
    distortion::DistortionMode,
};

/// Hubass synthesizer module.
pub mod hubass;
/// Hubass voice logic.
pub mod voice;

/// Patch parameters specifying a preset configuration for the versatile Rave/Bass `SynthHubass`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SynthHubassPatch {
    /// Amplitude ADSR envelope parameters [A, D, S, R].
    pub amp_adsr: [f32; 4],
    
    // Unison Engine parameters
    pub unison_waveform: Waveform,
    pub unison_voices: usize,
    pub unison_detune: f32,
    pub unison_spread: f32,
    pub unison_gain: f32,

    // Sub-Oscillator parameters
    pub sub_waveform: SubWaveform,
    pub sub_octave: i32, // -1 or -2
    pub sub_gain: f32,

    // Noise parameter
    pub noise_gain: f32,

    // Filter parameters
    pub filter_mode: i32, // 0: ZDF LP, 1: ZDF BP, 2: Formant Vowel
    pub filter_start_mult: f32,
    pub filter_cutoff_end: f32,
    pub filter_decay: f32,
    pub filter_resonance: f32,

    // Distortion/Drive parameters
    pub drive_mode: DistortionMode,
    pub drive_gain: f32,
    pub drive_mix: f32,

    // LFO 1 parameters
    pub lfo1_waveform: LfoWaveform,
    pub lfo1_speed: f32,
    pub lfo1_cutoff_depth: f32,
    pub lfo1_pitch_depth: f32,

    // Chorus parameters
    pub chorus_mix: f32,
    pub chorus_depth: f32,

    // Master Output Gain
    pub output_gain: f32,
}

impl Default for SynthHubassPatch {
    fn default() -> Self {
        Self {
            amp_adsr: [0.05, 0.2, 0.7, 0.3],
            unison_waveform: Waveform::Saw,
            unison_voices: 5,
            unison_detune: 0.035,
            unison_spread: 0.8,
            unison_gain: 0.8,
            sub_waveform: SubWaveform::Sine,
            sub_octave: -1,
            sub_gain: 0.7,
            noise_gain: 0.05,
            filter_mode: 0, // ZDF LP
            filter_start_mult: 1.333,
            filter_cutoff_end: 800.0,
            filter_decay: 1.0,
            filter_resonance: 0.4,
            drive_mode: DistortionMode::Tanh,
            drive_gain: 2.0,
            drive_mix: 0.5,
            lfo1_waveform: LfoWaveform::Sine,
            lfo1_speed: 1.5,
            lfo1_cutoff_depth: 0.0,
            lfo1_pitch_depth: 0.0,
            chorus_mix: 0.5,
            chorus_depth: 0.5,
            output_gain: 1.0,
        }
    }
}
