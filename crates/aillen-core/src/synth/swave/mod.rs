use crate::dsp::{
    oscillator::{Waveform, SubWaveform},
    distortion::DistortionMode,
};

/// Voice and playback logic.
pub mod voice;
/// Polyphonic SWAVE synth instrument wrapper.
pub mod swave;

/// Operating mode of the SWAVE synthesizer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SwaveMode {
    /// SuperWave Saw with center oscillator, detuned inner pair, detuned outer pair, and sub-oscillator.
    Saw,
    /// SuperWave Ensemble: 4-oscillator interval cluster for lush chords and multi-voice pads.
    Ensemble,
}

/// Patch parameters specifying a preset configuration for `SwaveSynth`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwavePatch {
    /// Operating synthesis mode (Saw vs Ensemble).
    pub mode: SwaveMode,

    // Base Oscillator / Tuning
    /// Center / base waveform (typically Saw, but supports Sine, Square, Triangle).
    pub base_waveform: Waveform,

    // Unison Parameters (SWAVE-SAW)
    /// Unison detune spread amount (UNIW), typically 0.0 to 0.1 (up to ~several semitones).
    pub unison_detune: f32,
    /// Level of the inner detuned oscillator pair (+/- detune). (UNID)
    pub unison_level: f32,
    /// Level of the outer/extended detuned oscillator pair (+/- 2*detune). (UNIX)
    pub unison_ext_level: f32,
    /// Stereo panning spread for the unison oscillators (0.0 = mono, 1.0 = wide stereo).
    pub stereo_spread: f32,

    // Ensemble Mode Parameters (SWAVE-ENS)
    /// Semitone offsets for ensemble voices 2, 3, and 4 relative to root.
    /// E.g. [3.0, 7.0, 10.0] for a minor 7th chord, or [0.05, -0.05, 12.0] for octave unison.
    pub ensemble_intervals: [f32; 3],
    /// Level of the ensemble voices (0.0 to 1.0).
    pub ensemble_level: f32,

    // Sub-Oscillator
    /// Sub-oscillator waveform (Square, Sine, Saw, Triangle).
    pub sub_waveform: SubWaveform,
    /// Sub-oscillator octave offset (-1 or -2).
    pub sub_octave: i32,
    /// Sub-oscillator output level (SUBD), 0.0 to 1.0.
    pub sub_gain: f32,

    // Monomachine Base & Width Filter
    /// Filter Base frequency in Hz (High-Pass cutoff).
    pub filter_base: f32,
    /// Filter Width span in Hz (Low-Pass cutoff = base + width).
    pub filter_width: f32,
    /// High-pass filter resonance Q.
    pub filter_hp_q: f32,
    /// Low-pass filter resonance Q.
    pub filter_lp_q: f32,
    /// Envelope modulation depth onto Base frequency in Hz.
    pub filter_env_amount: f32,

    // Envelopes
    /// Amplitude ADSR envelope parameters [A, D, S, R].
    pub amp_adsr: [f32; 4],
    /// Filter modulation ADSR envelope parameters [A, D, S, R].
    pub filter_adsr: [f32; 4],

    // Saturation / Distortion
    /// Saturation drive mode (SoftClip, HardClip, Tube, Fold).
    pub drive_mode: DistortionMode,
    /// Drive gain (1.0 = neutral, >1.0 = overdriven).
    pub drive_gain: f32,
    /// Drive dry/wet mix (0.0 to 1.0).
    pub drive_mix: f32,
}

impl Default for SwavePatch {
    fn default() -> Self {
        Self {
            mode: SwaveMode::Saw,
            base_waveform: Waveform::Saw,
            unison_detune: 0.025,
            unison_level: 0.75,
            unison_ext_level: 0.5,
            stereo_spread: 0.8,
            ensemble_intervals: [4.0, 7.0, 11.0], // Major 7th
            ensemble_level: 0.7,
            sub_waveform: SubWaveform::Square,
            sub_octave: -1,
            sub_gain: 0.4,
            filter_base: 20.0,
            filter_width: 16000.0,
            filter_hp_q: 0.707,
            filter_lp_q: 1.2,
            filter_env_amount: 4000.0,
            amp_adsr: [0.005, 0.2, 0.7, 0.3],
            filter_adsr: [0.01, 0.3, 0.2, 0.4],
            drive_mode: DistortionMode::Tanh,
            drive_gain: 1.5,
            drive_mix: 0.3,
        }
    }
}
