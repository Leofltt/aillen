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
}

impl Default for TwoOpPatch {
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
        }
    }
}
