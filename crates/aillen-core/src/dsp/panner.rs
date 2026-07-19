use std::f32::consts::FRAC_PI_2;
use crate::dsp::StereoProcessor;

/// Panning modes for stereo placement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PanMode {
    /// Constant Power Law using Sine/Cosine.
    /// Resulting power (L^2 + R^2) is constant across the pan range.
    /// Provides a smooth transition without a perceived volume dip in the center.
    ConstantPowerSin,
    
    /// Constant Power Law using Square Root.
    /// Similar to ConstantPowerSin but with a different curve shape.
    ConstantPowerSqrt,
    
    /// Mid/Side-style linear panning (Constant Amplitude).
    /// Sum of L + R is constant across the pan range.
    MidSide,
}

/// A panner for positioning a mono signal in a stereo field.
pub struct Panner {
    /// Pan position from -1.0 (Hard Left) to 1.0 (Hard Right).
    pub pan: f32,
    /// The panning law to apply.
    pub mode: PanMode,
}

impl Panner {
    /// Creates a new Panner with the given position and mode.
    pub fn new(pan: f32, mode: PanMode) -> Self {
        Self {
            pan: pan.clamp(-1.0, 1.0),
            mode,
        }
    }

    /// Sets the pan position, clamped to [-1.0, 1.0].
    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
    }

    /// Sets the panning mode.
    pub fn set_mode(&mut self, mode: PanMode) {
        self.mode = mode;
    }

    /// Returns the (Left, Right) gains based on current pan and mode.
    pub fn get_gains(&self) -> (f32, f32) {
        let normalized_pan = (self.pan + 1.0) * 0.5;

        match self.mode {
            PanMode::ConstantPowerSin => {
                let left_gain = ((1.0 - normalized_pan) * FRAC_PI_2).sin();
                let right_gain = (normalized_pan * FRAC_PI_2).sin();
                (left_gain, right_gain)
            }
            PanMode::ConstantPowerSqrt => {
                let left_gain = (1.0 - normalized_pan).sqrt();
                let right_gain = normalized_pan.sqrt();
                (left_gain, right_gain)
            }
            PanMode::MidSide => {
                let left_gain = 1.0 - normalized_pan;
                let right_gain = normalized_pan;
                (left_gain, right_gain)
            }
        }
    }

    /// Processes a mono input and returns a stereo (Left, Right) pair.
    pub fn process(&self, input: f32) -> (f32, f32) {
        let (left_gain, right_gain) = self.get_gains();
        (input * left_gain, input * right_gain)
    }
}

impl StereoProcessor for Panner {
    /// Positions a stereo input using pan-balance gains.
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (left_gain, right_gain) = self.get_gains();
        (left * left_gain, right * right_gain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panner_center() {
        let panner = Panner::new(0.0, PanMode::ConstantPowerSin);
        let (l, r) = panner.process(1.0);
        assert!((l - 0.70710677).abs() < 1e-6);
        assert!((r - 0.70710677).abs() < 1e-6);
    }

    #[test]
    fn test_panner_hard_left() {
        let panner = Panner::new(-1.0, PanMode::ConstantPowerSin);
        let (l, r) = panner.process(1.0);
        assert!((l - 1.0).abs() < 1e-6);
        assert!(r.abs() < 1e-6);
    }

    #[test]
    fn test_panner_hard_right() {
        let panner = Panner::new(1.0, PanMode::ConstantPowerSin);
        let (l, r) = panner.process(1.0);
        assert!(l.abs() < 1e-6);
        assert!((r - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_panner_sqrt_center() {
        let panner = Panner::new(0.0, PanMode::ConstantPowerSqrt);
        let (l, r) = panner.process(1.0);
        assert!((l - 0.70710677).abs() < 1e-6);
        assert!((r - 0.70710677).abs() < 1e-6);
    }

    #[test]
    fn test_panner_midside_center() {
        let panner = Panner::new(0.0, PanMode::MidSide);
        let (l, r) = panner.process(1.0);
        assert!((l - 0.5).abs() < 1e-6);
        assert!((r - 0.5).abs() < 1e-6);
    }
    
    #[test]
    fn test_panner_midside_hard_left() {
        let panner = Panner::new(-1.0, PanMode::MidSide);
        let (l, r) = panner.process(1.0);
        assert!((l - 1.0).abs() < 1e-6);
        assert!(r.abs() < 1e-6);
    }
}
