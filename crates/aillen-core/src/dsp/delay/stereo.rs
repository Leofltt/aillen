use crate::dsp::StereoProcessor;
use super::tape::TapeDelay;
use super::granular::GranularDelay;

/// Supported modes for the `StereoDelay`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DelayMode {
    /// Classic tape-style delay with fractional delay times and ping-pong feedback.
    Tape,
    /// Granular delay that slices input signals into windowed grain fragments.
    Granular,
}

/// A stereo delay processor that wraps both a tape-style delay and a granular delay.
/// Implements `StereoProcessor` to handle stereo dry/wet processing.
pub struct StereoDelay {
    /// Active delay mode.
    pub mode: DelayMode,
    /// Dry/wet mix (0.0 = completely dry, 1.0 = completely wet).
    pub mix: f32,
    /// Tape delay sub-module.
    pub tape: TapeDelay,
    /// Granular delay sub-module.
    pub granular: GranularDelay,
}

impl StereoDelay {
    /// Creates a new `StereoDelay` instance with both delay modes initialized.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            mode: DelayMode::Tape,
            mix: 0.5,
            tape: TapeDelay::new(sample_rate),
            granular: GranularDelay::new(sample_rate),
        }
    }
}

impl StereoProcessor for StereoDelay {
    /// Processes a stereo audio frame. Delegates the delay calculation to the active
    /// delay sub-module and blends the output with the dry input.
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (delayed_l, delayed_r) = match self.mode {
            DelayMode::Tape => self.tape.process_stereo(left, right),
            DelayMode::Granular => self.granular.process_stereo(left, right),
        };

        let out_l = left * (1.0 - self.mix) + delayed_l * self.mix;
        let out_r = right * (1.0 - self.mix) + delayed_r * self.mix;

        (out_l, out_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stereo_delay_dry_wet() {
        let mut delay = StereoDelay::new(44100.0);
        
        // 100% dry should pass inputs untouched
        delay.mix = 0.0;
        let (out_l, out_r) = delay.process_stereo(0.7, 0.7);
        assert!((out_l - 0.7).abs() < 1e-5);
        assert!((out_r - 0.7).abs() < 1e-5);
        
        // 100% wet with a zeroed delay buffer should return 0.0 initially
        delay.mix = 1.0;
        let (out_l2, out_r2) = delay.process_stereo(1.0, 1.0);
        assert!((out_l2 - 0.0).abs() < 1e-5);
        assert!((out_r2 - 0.0).abs() < 1e-5);
    }
}
