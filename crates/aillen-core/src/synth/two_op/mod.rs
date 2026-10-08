/// Polyphonic FM synthesizer engine.
pub mod two_op;
/// Monophonic/polyphonic voice logic.
pub mod voice;

use crate::dsp::{oscillator::Waveform, filter::FilterType};

/// Synthesis algorithms supported by the Two-Operator synth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SynthMode {
    /// Simply sum both operators.
    Additive,
    /// Amplitude Modulation (Modulator scales Carrier offset/gain).
    Am,
    /// Ring Modulation (Modulator multiplied by Carrier).
    Rm,
    /// Frequency Modulation (Modulator modulates Carrier frequency).
    Fm,
}

/// Patch parameters specifying a preset configuration for the `TwoOpSynth`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoOpPatch {
    /// The active synthesis mode.
    pub mode: SynthMode,
    /// Waveform of Operator 1 (Carrier).
    pub osc1_waveform: Waveform,
    /// Waveform of Operator 2 (Modulator).
    pub osc2_waveform: Waveform,
    /// ADSR parameters for Operator 1 [A, D, S, R].
    pub osc1_adsr: [f32; 4],
    /// ADSR parameters for Operator 2 [A, D, S, R].
    pub osc2_adsr: [f32; 4],
    /// ADSR parameters for Filter Cutoff [A, D, S, R].
    pub filter_adsr: [f32; 4],
    /// Base filter cutoff in Hz.
    pub filter_cutoff: f32,
    /// Filter resonance Q-factor.
    pub filter_q: f32,
    /// Biquad filter type.
    pub filter_type: FilterType,
    /// Whether envelope modulation of filter cutoff is enabled.
    pub filter_mod_enabled: bool,
    /// Filter envelope modulation depth in Hz.
    pub filter_env_amount: f32,
    /// Modulation index (modulation gain factor).
    pub modulation_index: f32,
    /// Operator 2 detune ratio.
    pub osc2_detune: f32,
    /// Operator 2 pitch multiplier ratio relative to Carrier.
    pub osc2_ratio: f32,
    
    // PM Feedback
    pub osc2_feedback: f32,
    /// Carrier (Operator 1) self-feedback amount.
    pub osc1_feedback: f32,
    
    // Wavefolder parameters
    pub wavefold_gain: f32,
    pub wavefold_mix: f32,

    // Phase Noise parameters
    pub carrier_noise: f32,
    pub modulator_noise: f32,

    // Pitch sweep parameters
    pub pitch_sweep_depth: f32, // in semitones (e.g. -48.0 to 48.0)
    pub pitch_sweep_decay: f32, // in seconds

    // Voice LFO parameters
    pub lfo_waveform: usize,
    pub lfo_speed: f32,
    pub lfo_mod_index: f32,
    pub lfo_cutoff: f32,

    // Monomachine FM+ style quantized ratio mode
    /// Whether operator 2 ratio is quantized to musical harmonic intervals (like Monomachine FM+STATIC).
    pub ratio_quantize: bool,
    /// Static ratio table index (0..16) when quantize is active.
    pub ratio_index: usize,

    // Monomachine Base/Width filter mode
    /// Whether to use the Monomachine-style Base & Width dual HP/LP filter instead of the standard biquad.
    pub base_width_enabled: bool,
    /// Base HP cutoff frequency in Hz.
    pub filter_base: f32,
    /// Width passband span in Hz (LP cutoff = base + width).
    pub filter_width: f32,
    /// HP filter resonance Q.
    pub filter_hp_q: f32,
}

/// Harmonic ratio table inspired by classic FM synths and the Monomachine FM+STATIC machine.
pub const HARMONIC_RATIOS: [f32; 17] = [
    0.125, // 1/8
    0.25,  // 1/4
    0.5,   // 1/2
    0.75,  // 3/4
    1.0,   // 1
    1.25,  // 5/4
    1.5,   // 3/2
    2.0,   // 2
    2.5,   // 5/2
    3.0,   // 3
    3.5,   // 7/2
    4.0,   // 4
    5.0,   // 5
    6.0,   // 6
    7.0,   // 7
    8.0,   // 8
    12.0,  // 12
];

impl TwoOpPatch {
    /// Returns the default patch configuration.
    fn default() -> Self {
        Self {
            mode: SynthMode::Additive,
            osc1_waveform: Waveform::Saw,
            osc2_waveform: Waveform::Saw,
            osc1_adsr: [0.01, 0.2, 0.5, 0.5],
            osc2_adsr: [0.01, 0.2, 0.5, 0.5],
            filter_adsr: [0.05, 0.3, 0.2, 0.5],
            filter_cutoff: 1000.0,
            filter_q: 0.707,
            filter_type: FilterType::LowPass,
            filter_mod_enabled: true,
            filter_env_amount: 5000.0,
            modulation_index: 1.0,
            osc2_detune: 0.0,
            osc2_ratio: 1.0,
            osc2_feedback: 0.0,
            osc1_feedback: 0.0,
            wavefold_gain: 1.0,
            wavefold_mix: 0.0,
            carrier_noise: 0.0,
            modulator_noise: 0.0,
            pitch_sweep_depth: 0.0,
            pitch_sweep_decay: 0.1,
            lfo_waveform: 0,
            lfo_speed: 2.0,
            lfo_mod_index: 0.0,
            lfo_cutoff: 0.0,
            ratio_quantize: false,
            ratio_index: 4, // 1.0
            base_width_enabled: false,
            filter_base: 20.0,
            filter_width: 20000.0,
            filter_hp_q: 0.707,
        }
    }
}
