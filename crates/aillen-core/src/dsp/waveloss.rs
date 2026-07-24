use crate::dsp::AudioProcessor;

/// A DSP module implementing the WaveLoss algorithm.
/// It splits the audio stream into segments based on zero-crossings
/// and discards a fraction of them, replacing them with silence.
pub struct WaveLoss {
    /// Number of segments to drop in each cycle.
    pub drop: usize,
    /// Total number of segments in a cycle.
    pub outof: usize,
    /// Mode: 1 = deterministic (pos >= drop), 2 = random (probabilistic drop).
    pub mode: usize,
    // Internal states
    on: bool,
    pos: usize,
    prevval: f32,
    rng_state: u32,
}

impl WaveLoss {
    /// Creates a new `WaveLoss` processor.
    /// Default values are set to bypass the effect (`drop = 0`, `outof = 40`).
    pub fn new() -> Self {
        Self {
            drop: 0,
            outof: 40,
            mode: 1,
            on: true,
            pos: 0,
            prevval: 0.0,
            rng_state: 12345, // simple seed
        }
    }
}

impl Default for WaveLoss {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioProcessor for WaveLoss {
    fn process(&mut self, input: f32) -> f32 {
        if self.drop == 0 || self.outof == 0 {
            return input;
        }

        let curval = input;
        
        // Check for positive-going zero-crossing
        if self.prevval < 0.0 && curval >= 0.0 {
            self.pos += 1;
            if self.pos >= self.outof {
                self.pos = 0;
            }

            if self.mode == 2 {
                // Random mode: drop segments with probability (drop / outof)
                // Linear Congruential Generator (LCG) for fast pseudo-random values in [0.0, 1.0)
                self.rng_state = self.rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                let rand_val = (self.rng_state & 0x7FFF_FFFF) as f32 / 2147483647.0;
                let threshold = self.drop as f32 / self.outof as f32;
                self.on = rand_val >= threshold;
            } else {
                // Deterministic mode: keep segments if pos >= drop
                self.on = self.pos >= self.drop;
            }
        }

        self.prevval = curval;

        if self.on {
            curval
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveloss_bypass() {
        let mut wl = WaveLoss::new();
        // By default, drop is 0, so it shouldn't modify anything
        for val in &[-0.5, 0.5, 1.0, -1.0, 0.0] {
            assert_eq!(wl.process(*val), *val);
        }
    }

    #[test]
    fn test_waveloss_deterministic_drop() {
        let mut wl = WaveLoss::new();
        wl.drop = 1;
        wl.outof = 2;
        wl.mode = 1;

        // Sequence of positive-going zero crossings:
        // Transition 1: -0.5 -> 0.5 (crossing 1) -> pos = 1 (on = pos >= 1 -> true)
        // Transition 2: -0.5 -> 0.5 (crossing 2) -> pos = 2 -> pos = 0 (on = pos >= 1 -> false)
        
        wl.process(-0.5);
        let out1 = wl.process(0.5);
        assert_eq!(out1, 0.5); // pos = 1, on = true

        wl.process(-0.5);
        let out2 = wl.process(0.5);
        assert_eq!(out2, 0.0); // pos = 0, on = false
    }
}
