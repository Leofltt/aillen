use std::f32::consts::PI;
use crate::dsp::AudioNode;
use crate::dsp::oscillator::Waveform;

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
