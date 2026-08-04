use crate::dsp::AudioProcessor;

/// A digital audio degrader applying bit-depth quantization reduction and sample-and-hold downsampling.
pub struct Bitcrusher {
    /// Quantization bit depth (1.0 to 16.0, 16.0 = bypass)
    pub bits: f32,
    /// Sample rate divider (1 = bypass, >1 = hold samples for N steps)
    pub downsample: usize,

    sample_count: usize,
    held_sample: f32,
}

impl Bitcrusher {
    pub fn new(bits: f32, downsample: usize) -> Self {
        Self {
            bits: bits.clamp(1.0, 16.0),
            downsample: downsample.max(1),
            sample_count: 0,
            held_sample: 0.0,
        }
    }
}

impl Default for Bitcrusher {
    fn default() -> Self {
        Self {
            bits: 16.0,
            downsample: 1,
            sample_count: 0,
            held_sample: 0.0,
        }
    }
}

impl AudioProcessor for Bitcrusher {
    fn process(&mut self, input: f32) -> f32 {
        // Bypass check
        if self.bits >= 15.99 && self.downsample <= 1 {
            return input;
        }

        // 1. Sample-and-Hold Downsampling
        if self.sample_count == 0 {
            self.held_sample = input;
        }
        self.sample_count = (self.sample_count + 1) % self.downsample.max(1);

        let sampled = self.held_sample;

        // 2. Bit Depth Quantization
        if self.bits < 15.99 {
            let steps = 2.0_f32.powf(self.bits.clamp(1.0, 16.0));
            (sampled * steps).round() / steps
        } else {
            sampled
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitcrusher_bypass() {
        let mut bc = Bitcrusher::default();
        assert_eq!(bc.process(0.54321), 0.54321);
    }

    #[test]
    fn test_bitcrusher_quantize() {
        let mut bc = Bitcrusher::new(4.0, 1);
        let out = bc.process(0.33);
        // 4 bits = 16 steps
        assert!((out - 0.3125).abs() < 1e-4);
    }
}
