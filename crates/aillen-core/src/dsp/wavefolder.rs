use crate::dsp::AudioProcessor;

/// A non-linear wavefolder effect that folds waveforms exceeding a threshold back on themselves.
/// Useful for generating rich, glassy, bright harmonics.
pub struct Wavefolder {
    /// Input drive gain multiplier (>= 1.0)
    pub drive: f32,
    /// Folding intensity / iteration factor (0.0 = bypass, > 0.0 = active folding)
    pub folds: f32,
    /// Symmetry / DC offset shift before folding (-1.0 to 1.0)
    pub symmetry: f32,
}

impl Wavefolder {
    pub fn new(drive: f32, folds: f32, symmetry: f32) -> Self {
        Self { drive, folds, symmetry }
    }
}

impl Default for Wavefolder {
    fn default() -> Self {
        Self {
            drive: 1.0,
            folds: 0.0, // Default off / bypass
            symmetry: 0.0,
        }
    }
}

impl AudioProcessor for Wavefolder {
    fn process(&mut self, input: f32) -> f32 {
        if self.folds <= 1e-5 {
            return input;
        }

        // Apply drive gain and symmetry DC offset
        let driven = (input * self.drive) + self.symmetry;

        // Trigonometric sine-fold algorithm scaled by folds parameter
        let folded = (driven * (1.0 + self.folds)).sin();

        // Blend dry input with folded wet signal based on folds parameter clamped to [0, 1]
        let mix = self.folds.clamp(0.0, 1.0);
        let output = input * (1.0 - mix) + folded * mix;

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
    fn test_wavefolder_bypass() {
        let mut wf = Wavefolder::default();
        assert_eq!(wf.process(0.5), 0.5);
        assert_eq!(wf.process(-0.8), -0.8);
    }

    #[test]
    fn test_wavefolder_active() {
        let mut wf = Wavefolder::new(2.0, 1.0, 0.0);
        let out = wf.process(0.8);
        assert_ne!(out, 0.8);
    }
}
