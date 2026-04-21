use std::f32::consts::PI;
use super::AudioNode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

pub struct NaiveOscillator {
    pub sample_rate: f32,
    pub frequency: f32,
    pub waveform: Waveform,
    phase: f32,
}

impl NaiveOscillator {
    pub fn new(sample_rate: f32, frequency: f32, waveform: Waveform) -> Self {
        Self {
            sample_rate,
            frequency,
            waveform,
            phase: 0.0,
        }
    }
    
    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }
}

impl AudioNode for NaiveOscillator {
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

pub struct PolyBlepOscillator {
    pub sample_rate: f32,
    pub frequency: f32,
    pub waveform: Waveform,
    phase: f32,
}

impl PolyBlepOscillator {
    pub fn new(sample_rate: f32, frequency: f32, waveform: Waveform) -> Self {
        Self {
            sample_rate,
            frequency,
            waveform,
            phase: 0.0,
        }
    }
    
    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }
    
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
                // Triangle aliasing is much lower amplitude so naive is generally acceptable,
                // but integrating a polyblep square yields an optimal triangle.
                // We'll stick to naive here for performance.
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
