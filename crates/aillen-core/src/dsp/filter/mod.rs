/// Standard 2-pole biquad IIR filter implementation.
pub mod biquad;
/// DJ performance style low-pass/high-pass combined filter.
pub mod dj;
/// Resonant 4-pole ZDF ladder filter.
pub mod ladder;
/// Formant filter implementation.
pub mod formant;
/// Comb filter implementation.
pub mod comb;
/// Monomachine-inspired Base & Width serial high-pass/low-pass filter.
pub mod base_width;

pub use biquad::{BiquadFilter, FilterType};
pub use dj::DjFilter;
pub use ladder::ResonantLadderFilter;
pub use formant::FormantFilter;
pub use comb::CombFilter;
pub use base_width::BaseWidthFilter;
