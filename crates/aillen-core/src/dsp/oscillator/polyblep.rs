use std::f32::consts::PI;
use crate::dsp::AudioNode;
use crate::dsp::oscillator::Waveform;

/// An anti-aliased oscillator using PolyBLEP (Polynomial Band-Limited Step) residual correction.
/// Recommended for high-frequency synth voices to eliminate digital aliasing noise.
pub struct PolyBlepOscillator {
    /// Active sample rate in Hz.
    pub sample_rate: f32,
    /// Current oscillator frequency in Hz.
    pub frequency: f32,
    /// Selected waveform.
    pub waveform: Waveform,
    /// Internal phase accumulator from 0.0 to 1.0.
    phase: f32,
}

impl PolyBlepOscillator {
    /// Creates a new `PolyBlepOscillator`.
    pub fn new(sample_rate: f32, frequency: f32, waveform: Waveform) -> Self {
        Self {
            sample_rate,
            frequency,
            waveform,
            phase: 0.0,
        }
    }
    
    /// Updates the oscillator's target frequency.
    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    /// Updates the oscillator's waveform.
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }
    
    /// Calculates the PolyBLEP step correction factor.
    fn poly_blep(t: f32, dt: f32) -> f32 {
        if t < dt {
            let t = t / dt;
            t + t - t * t - 1.0
        } else if t > 1.0 - dt {
            let t = (t - 1.0) / dt;
            t * t + t + t + 1.0
        } else {
            0.0
        }
    }
}

impl AudioNode for PolyBlepOscillator {
    /// Computes and returns the next band-limited sample frame.
    fn process(&mut self) -> f32 {
        let dt = self.frequency / self.sample_rate;
        let t = self.phase;
        
        let sample = match self.waveform {
            Waveform::Sine => {
                (t * 2.0 * PI).sin()
            }
            Waveform::Saw => {
                let naive = 2.0 * t - 1.0;
                naive - Self::poly_blep(t, dt)
            }
            Waveform::Square => {
                let naive = if t < 0.5 { 1.0 } else { -1.0 };
                let mut shifted_t = t + 0.5;
                if shifted_t >= 1.0 {
                    shifted_t -= 1.0;
                }
                naive + Self::poly_blep(t, dt) - Self::poly_blep(shifted_t, dt)
            }
            Waveform::Triangle => {
                1.0 - 4.0 * (t - 0.5).abs()
            }
        };
        
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        
        sample
    }
}
