use crate::dsp::AudioProcessor;
use super::biquad::{BiquadFilter, FilterType};

/// A DJ-style performance filter with a single control parameter.
/// - At position `0.0`: The filter is bypassed (passes input unchanged).
/// - From `0.0` down to `-1.0`: Acts as a Low-Pass filter, sweeping cutoff from 20000 Hz down to 20 Hz.
/// - From `0.0` up to `1.0`: Acts as a High-Pass filter, sweeping cutoff from 20 Hz up to 20000 Hz.
pub struct DjFilter {
    lp_filter: BiquadFilter,
    hp_filter: BiquadFilter,
    /// Parameter range: -1.0 to 1.0.
    pub position: f32,
}

impl DjFilter {
    /// Creates a new `DjFilter` initialized at the bypass center position (0.0).
    pub fn new(sample_rate: f32) -> Self {
        Self {
            lp_filter: BiquadFilter::new_lowpass(sample_rate, 20000.0, 0.707),
            hp_filter: BiquadFilter::new(sample_rate, 20.0, 0.707, FilterType::HighPass),
            position: 0.0,
        }
    }

    /// Sets the position of the DJ filter, clamped to [-1.0, 1.0].
    /// Updates the internal filter cutoffs using exponential curves for a natural frequency sweep.
    pub fn set_position(&mut self, pos: f32) {
        self.position = pos.clamp(-1.0, 1.0);
        
        if self.position < 0.0 {
            // Low-pass mode. Map position [-1.0, 0.0] exponentially to [20.0, 20000.0]
            let norm = self.position + 1.0; // 0.0 to 1.0
            let cutoff = 20.0 * (1000.0_f32).powf(norm); // 20.0 * 1000.0^norm = 20.0 to 20000.0
            self.lp_filter.set_cutoff(cutoff);
        } else if self.position > 0.0 {
            // High-pass mode. Map position [0.0, 1.0] exponentially to [20.0, 20000.0]
            let norm = self.position; // 0.0 to 1.0
            let cutoff = 20.0 * (1000.0_f32).powf(norm); // 20.0 * 1000.0^norm = 20.0 to 20000.0
            self.hp_filter.set_cutoff(cutoff);
        }
    }
}

impl AudioProcessor for DjFilter {
    /// Processes a single input sample through the active filter depending on the current control position.
    fn process(&mut self, input: f32) -> f32 {
        if self.position < 0.0 {
            self.lp_filter.process(input)
        } else if self.position > 0.0 {
            self.hp_filter.process(input)
        } else {
            input
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dj_filter_bypass() {
        let mut filter = DjFilter::new(44100.0);
        filter.set_position(0.0);
        
        // At bypass, input should match output exactly
        for i in 0..10 {
            let val = i as f32 * 0.1;
            assert_eq!(filter.process(val), val);
        }
    }

    #[test]
    fn test_dj_filter_lowpass() {
        let mut filter = DjFilter::new(44100.0);
        filter.set_position(-0.5);
        
        // Low-pass mode should attenuate high frequencies (alternating +1/-1)
        let mut out = 0.0;
        for i in 0..100 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            out = filter.process(input);
        }
        // At -0.5 position, high frequencies should be heavily attenuated
        assert!(out.abs() < 0.2);
    }
}
