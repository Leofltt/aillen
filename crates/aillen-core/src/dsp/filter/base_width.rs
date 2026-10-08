use super::biquad::{BiquadFilter, FilterType};
use crate::dsp::AudioProcessor;

/// A serialized High-Pass and Low-Pass filter controlled via Base and Width parameters,
/// inspired by the Elektron Monomachine filter architecture.
///
/// - `base`: Cutoff frequency of the High-Pass filter in Hz.
/// - `width`: Pass-band width in Hz.
/// - The Low-Pass filter cutoff is computed as `base + width`.
/// - Sweeping `base` shifts the entire passband up/down while maintaining the band window.
pub struct BaseWidthFilter {
    sample_rate: f32,
    pub base: f32,
    pub width: f32,
    pub hp_q: f32,
    pub lp_q: f32,
    hp_filter: BiquadFilter,
    lp_filter: BiquadFilter,
}

impl BaseWidthFilter {
    /// Creates a new `BaseWidthFilter` with default settings (Base = 20 Hz, Width = 20000 Hz, transparent bandpass).
    pub fn new(sample_rate: f32) -> Self {
        let base = 20.0;
        let width = 20000.0;
        let hp_q = 0.707;
        let lp_q = 0.707;

        let hp_filter = BiquadFilter::new(sample_rate, base, hp_q, FilterType::HighPass);
        let lp_filter = BiquadFilter::new(
            sample_rate,
            (base + width).min(sample_rate * 0.49),
            lp_q,
            FilterType::LowPass,
        );

        Self {
            sample_rate,
            base,
            width,
            hp_q,
            lp_q,
            hp_filter,
            lp_filter,
        }
    }

    /// Sets the Base (HP cutoff) and Width (passband width) frequencies in Hz.
    pub fn set_base_width(&mut self, base: f32, width: f32) {
        self.base = base.clamp(10.0, self.sample_rate * 0.48);
        self.width = width.max(10.0);
        let lp_cutoff = (self.base + self.width).clamp(self.base + 5.0, self.sample_rate * 0.49);
        self.hp_filter.set_cutoff(self.base);
        self.lp_filter.set_cutoff(lp_cutoff);
    }

    /// Sets the resonance Q factors for both the High-Pass and Low-Pass sections.
    pub fn set_resonance(&mut self, hp_q: f32, lp_q: f32) {
        self.hp_q = hp_q.clamp(0.1, 20.0);
        self.lp_q = lp_q.clamp(0.1, 20.0);
        self.hp_filter.set_q(self.hp_q);
        self.lp_filter.set_q(self.lp_q);
    }
}

impl AudioProcessor for BaseWidthFilter {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let hp_out = self.hp_filter.process(input);
        self.lp_filter.process(hp_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_width_filter() {
        let mut filter = BaseWidthFilter::new(44100.0);
        filter.set_base_width(100.0, 1000.0);
        let mut out = 0.0;
        for _ in 0..100 {
            out = filter.process(0.5);
        }
        assert!(out.is_finite());
    }
}
