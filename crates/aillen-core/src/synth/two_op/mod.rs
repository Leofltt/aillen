pub mod two_op;
pub mod voice;

use crate::dsp::{oscillator::Waveform, filter::FilterType};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SynthMode {
    Additive,
    Am,
    Rm,
    Fm,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoOpPatch {
    pub mode: SynthMode,
    pub osc1_waveform: Waveform,
    pub osc2_waveform: Waveform,
    pub osc1_adsr: [f32; 4],
    pub osc2_adsr: [f32; 4],
    pub filter_adsr: [f32; 4],
    pub filter_cutoff: f32,
    pub filter_q: f32,
    pub filter_type: FilterType,
    pub filter_mod_enabled: bool,
    pub filter_env_amount: f32,
    pub modulation_index: f32,
    pub osc2_detune: f32,
    pub osc2_ratio: f32,
}

impl Default for TwoOpPatch {
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
