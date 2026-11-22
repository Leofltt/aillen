use crate::dsp::filter::biquad::{BiquadFilter, FilterType};
use crate::dsp::AudioProcessor;

#[inline]
fn clamp_finite(val: f32, min: f32, max: f32, fallback: f32) -> f32 {
    if val.is_finite() {
        val.clamp(min, max)
    } else {
        fallback
    }
}

/// A pitch-tracked, feedback-damped Comb Filter for physical acoustic/metallic resonance.
pub struct CombFilter {
    buffer: Vec<f32>,
    mask: usize,
    write_pos: usize,
    sample_rate: f32,
    was_active: bool,

    pub frequency: f32,
    pub feedback: f32,
    pub mix: f32,
    pub dampening_filter: BiquadFilter,
}

impl CombFilter {
    pub fn new(sample_rate: f32, frequency: f32, feedback: f32, dampening_cutoff: f32) -> Self {
        let sr = clamp_finite(sample_rate, 1000.0, 384000.0, 44100.0);
        // Allocate buffer for frequencies down to 20Hz rounded up to power of two
        let min_samples = (sr / 20.0).ceil() as usize + 2;
        let capacity = min_samples.max(4).next_power_of_two();
        let max_damp = (sr * 0.5 - 1.0).max(20.0);
        let damp = clamp_finite(dampening_cutoff, 20.0, max_damp, 8000.0_f32.min(max_damp));
        let dampening_filter = BiquadFilter::new(sr, damp, 0.707, FilterType::LowPass);

        Self {
            buffer: vec![0.0; capacity],
            mask: capacity - 1,
            write_pos: 0,
            sample_rate: sr,
            was_active: false,
            frequency: clamp_finite(frequency, 20.0, 10000.0, 440.0),
            feedback: clamp_finite(feedback, -0.99, 0.99, 0.0),
            mix: 0.0,
            dampening_filter,
        }
    }

    pub fn set_frequency(&mut self, freq: f32) {
        self.frequency = clamp_finite(freq, 20.0, 10000.0, 440.0);
    }

    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = clamp_finite(feedback, -0.99, 0.99, 0.0);
    }

    pub fn set_damp(&mut self, damp: f32) {
        let max_damp = (self.sample_rate * 0.5 - 1.0).max(20.0);
        let clamped = clamp_finite(damp, 20.0, max_damp, 8000.0_f32.min(max_damp));
        self.dampening_filter.set_cutoff(clamped);
    }

    pub fn set_mix(&mut self, mix: f32) {
        self.mix = clamp_finite(mix, 0.0, 1.0, 0.0);
    }
}

impl Default for CombFilter {
    fn default() -> Self {
        Self::new(44100.0, 440.0, 0.0, 8000.0)
    }
}

impl AudioProcessor for CombFilter {
    fn process(&mut self, input: f32) -> f32 {
        let mix = clamp_finite(self.mix, 0.0, 1.0, 0.0);

        // Bypass cleanly when dry/wet mix is negligible
        if mix < 1e-4 {
            if self.was_active {
                self.buffer.fill(0.0);
                self.dampening_filter.reset();
                self.was_active = false;
            }
            return input;
        }
        self.was_active = true;

        let freq = clamp_finite(self.frequency, 20.0, 10000.0, 440.0);
        let feedback = clamp_finite(self.feedback, -0.99, 0.99, 0.0);

        let delay_samples = (self.sample_rate / freq).clamp(1.0, (self.buffer.len() - 2) as f32);
        let delay_int = delay_samples.floor() as usize;
        let delay_frac = delay_samples - (delay_int as f32);

        let read_pos1 = (self.write_pos + self.buffer.len() - delay_int) & self.mask;
        let read_pos2 = (read_pos1 + self.buffer.len() - 1) & self.mask;

        // Linear interpolation read
        let delayed_raw = self.buffer[read_pos1] * (1.0 - delay_frac) + self.buffer[read_pos2] * delay_frac;

        // Dampening filter inside the loop
        let delayed_damped = self.dampening_filter.process(delayed_raw);

        // Feedback calculation
        let feed_sample = input + delayed_damped * feedback;
        self.buffer[self.write_pos] = feed_sample;

        self.write_pos = (self.write_pos + 1) & self.mask;

        // Output dry/wet crossfade
        let output = input * (1.0 - mix) + delayed_damped * mix;
        if output.abs() < 1e-15 {
            0.0
        } else {
            output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comb_filter_bypass() {
        let mut cf = CombFilter::new(44100.0, 440.0, 0.9, 8000.0);
        assert_eq!(cf.mix, 0.0);
        assert_eq!(cf.process(0.5), 0.5);
    }

    #[test]
    fn test_comb_filter_clamping() {
        let mut cf = CombFilter::new(44100.0, 5.0, 2.5, 50000.0);
        assert_eq!(cf.frequency, 20.0);
        assert_eq!(cf.feedback, 0.99);
        assert!(cf.dampening_filter.cutoff < 22050.0);

        cf.set_frequency(25000.0);
        assert_eq!(cf.frequency, 10000.0);
        cf.set_frequency(f32::NAN);
        assert_eq!(cf.frequency, 440.0);

        cf.set_feedback(-1.5);
        assert_eq!(cf.feedback, -0.99);
        cf.set_feedback(f32::NAN);
        assert_eq!(cf.feedback, 0.0);

        cf.set_mix(1.8);
        assert_eq!(cf.mix, 1.0);
        cf.set_mix(-0.2);
        assert_eq!(cf.mix, 0.0);
        cf.set_mix(f32::NAN);
        assert_eq!(cf.mix, 0.0);

        cf.set_damp(5.0);
        assert_eq!(cf.dampening_filter.cutoff, 20.0);
        cf.set_damp(f32::NAN);
        assert!((cf.dampening_filter.cutoff - 8000.0).abs() < 0.01);
    }

    #[test]
    fn test_comb_filter_dry_wet_and_reset_on_bypass() {
        // 44100 Hz / 441 Hz = 100 samples exact delay; feedback = 0.0 tests feedforward comb.
        let mut cf = CombFilter::new(44100.0, 441.0, 0.0, 20000.0);
        cf.set_mix(0.5);

        let y0 = cf.process(1.0);
        assert!((y0 - 0.5).abs() < 1e-6);

        let mut peak = 0.0_f32;
        for _ in 1..=105 {
            peak = peak.max(cf.process(0.0).abs());
        }
        assert!(peak > 0.2, "expected delayed feedforward comb pulse, got {peak}");

        // Turning mix back to 0 should flush the delay line and filter state.
        cf.set_feedback(0.8);
        cf.set_mix(1.0);
        for _ in 0..20 {
            cf.process(1.0);
        }
        cf.set_mix(0.0);
        assert_eq!(cf.process(0.25), 0.25);

        cf.set_mix(1.0);
        assert_eq!(cf.process(0.0), 0.0);
    }
}
