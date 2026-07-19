use std::f32::consts::PI;
use super::AudioNode;

/// Supported oscillator waveforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    /// Pure Sine wave.
    Sine,
    /// Sawtooth wave.
    Saw,
    /// Square wave.
    Square,
    /// Triangle wave.
    Triangle,
}

/// A naive oscillator implementation which contains aliasing harmonics.
/// Useful for low-frequency oscillators (LFOs) or when alias distortion is desired.
pub struct NaiveOscillator {
    /// Active sample rate in Hz.
    pub sample_rate: f32,
    /// Current oscillator frequency in Hz.
    pub frequency: f32,
    /// Selected waveform.
    pub waveform: Waveform,
    /// Internal accumulator phase from 0.0 to 1.0.
    phase: f32,
}

impl NaiveOscillator {
    /// Creates a new `NaiveOscillator`.
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
}

impl AudioNode for NaiveOscillator {
    /// Computes and returns the next output sample frame.
    fn process(&mut self) -> f32 {
        let phase_increment = self.frequency / self.sample_rate;
        let t = self.phase;
        
        let sample = match self.waveform {
            Waveform::Sine => (t * 2.0 * PI).sin(),
            Waveform::Saw => 2.0 * t - 1.0,
            Waveform::Square => if t < 0.5 { 1.0 } else { -1.0 },
            Waveform::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
        };
        
        self.phase += phase_increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        
        sample
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_oscillator() {
        let mut osc = NaiveOscillator::new(44100.0, 440.0, Waveform::Sine);
        let sample1 = osc.process();
        assert!(sample1.abs() < 1e-6);
        
        let sample2 = osc.process();
        assert!(sample2 > 0.0);
    }

    #[test]
    fn test_polyblep_sine() {
        let mut osc = PolyBlepOscillator::new(44100.0, 440.0, Waveform::Sine);
        let sample1 = osc.process();
        assert!(sample1.abs() < 1e-6);
    }
}
