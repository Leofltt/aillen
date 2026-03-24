use std::f32::consts::PI;
use super::AudioNode;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}

pub struct Oscillator {
    pub sample_rate: f32,
    pub frequency: f32,
    pub waveform: Waveform,
    phase: f32,
}

impl Oscillator {
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

impl AudioNode for Oscillator {
    fn process(&mut self) -> f32 {
        let phase_increment = self.frequency / self.sample_rate;
        let t = self.phase;
        
        // Generate sample
        let sample = match self.waveform {
            Waveform::Sine => (t * 2.0 * PI).sin(),
            Waveform::Saw => 2.0 * (t - t.floor()) - 1.0,
            Waveform::Square => if t < 0.5 { 1.0 } else { -1.0 },
            Waveform::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
        };
        
        // Advance phase
        self.phase += phase_increment;
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
        let mut osc = Oscillator::new(44100.0, 440.0, Waveform::Sine);
        let sample1 = osc.process();
        // The first sample is exactly at phase 0, so sine(0) = 0.
        assert!(sample1.abs() < 1e-6);
        
        let sample2 = osc.process();
        // Since frequency is positive, the next sample should be positive.
        assert!(sample2 > 0.0);
    }

    #[test]
    fn test_square_oscillator() {
        let mut osc = Oscillator::new(44100.0, 440.0, Waveform::Square);
        let sample = osc.process();
        // Square wave starts at 1.0 for first half of the cycle
        assert_eq!(sample, 1.0);
    }
}
