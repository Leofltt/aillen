use crate::dsp::AudioProcessor;
use std::f32::consts::PI;

/// Supported distortion algorithms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DistortionMode {
    Bypass = 0,
    Tanh = 1,
    HardClip = 2,
    Foldback = 3,
}

impl DistortionMode {
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => DistortionMode::Tanh,
            2 => DistortionMode::HardClip,
            3 => DistortionMode::Foldback,
            _ => DistortionMode::Bypass,
        }
    }
}

/// A reusable distortion/waveshaping effect.
pub struct Distortion {
    pub mode: DistortionMode,
    /// Gain multiplier applied before wave shaping.
    pub drive: f32,
    /// Post-shaping output level gain.
    pub mix: f32,
}

impl Distortion {
    pub fn new(mode: DistortionMode, drive: f32, mix: f32) -> Self {
        Self {
            mode,
            drive,
            mix,
        }
    }
}

impl AudioProcessor for Distortion {
    fn process(&mut self, input: f32) -> f32 {
        if self.mode == DistortionMode::Bypass {
            return input;
        }

        // Apply input drive
        let driven = input * self.drive;

        let shaped = match self.mode {
            DistortionMode::Bypass => driven,
            DistortionMode::Tanh => driven.tanh(),
            DistortionMode::HardClip => driven.clamp(-1.0, 1.0),
            DistortionMode::Foldback => {
                // Sinusoidal wavefolding for rich metallic folding textures
                (driven * (PI / 2.0)).sin()
            }
        };

        // Mix dry/wet
        let output = input * (1.0 - self.mix) + shaped * self.mix;
        
        // Prevent denormals
        if output.abs() < 1e-15 { 0.0 } else { output }
    }
}
