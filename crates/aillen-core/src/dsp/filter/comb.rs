use crate::dsp::filter::biquad::{BiquadFilter, FilterType};
use crate::dsp::AudioProcessor;

/// A pitch-tracked, feedback-damped Comb Filter for physical acoustic/metallic resonance.
pub struct CombFilter {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,

    pub frequency: f32,
    pub feedback: f32,
    pub dampening_filter: BiquadFilter,
}

impl CombFilter {
    pub fn new(sample_rate: f32, frequency: f32, feedback: f32, dampening_cutoff: f32) -> Self {
        // Allocate buffer for frequencies down to 20Hz
        let max_delay_samples = (sample_rate / 20.0).ceil() as usize + 2;
        let dampening_filter = BiquadFilter::new(sample_rate, dampening_cutoff, 0.707, FilterType::LowPass);

        Self {
            buffer: vec![0.0; max_delay_samples],
            write_pos: 0,
            sample_rate,
            frequency: frequency.clamp(20.0, 10000.0),
            feedback: feedback.clamp(-0.99, 0.99),
            dampening_filter,
        }
    }
}

impl Default for CombFilter {
    fn default() -> Self {
        Self::new(44100.0, 440.0, 0.0, 8000.0)
    }
}

impl AudioProcessor for CombFilter {
    fn process(&mut self, input: f32) -> f32 {
        // Bypass if feedback is negligible
        if self.feedback.abs() < 1e-4 {
            return input;
        }

        let delay_samples = (self.sample_rate / self.frequency.max(20.0)).clamp(1.0, (self.buffer.len() - 1) as f32);
        let delay_int = delay_samples.floor() as usize;
        let delay_frac = delay_samples - (delay_int as f32);

        let buf_len = self.buffer.len();
        let read_pos1 = (self.write_pos + buf_len - delay_int) % buf_len;
        let read_pos2 = (self.write_pos + buf_len - delay_int - 1) % buf_len;

        // Linear interpolation read
        let delayed_raw = self.buffer[read_pos1] * (1.0 - delay_frac) + self.buffer[read_pos2] * delay_frac;

        // Dampening filter inside the loop
        let delayed_damped = self.dampening_filter.process(delayed_raw);

        // Feedback calculation
        let feed_sample = input + delayed_damped * self.feedback;
        self.buffer[self.write_pos] = feed_sample;

        self.write_pos = (self.write_pos + 1) % buf_len;

        // Output mix
        let output = input + delayed_damped;
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
        let mut cf = CombFilter::new(44100.0, 440.0, 0.0, 8000.0);
        assert_eq!(cf.process(0.5), 0.5);
    }
}
