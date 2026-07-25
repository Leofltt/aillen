use std::f32::consts::PI;
use crate::dsp::AudioNode;

/// Waveshapes supported by the SubOscillator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SubWaveform {
    Sine,
    Triangle,
    Square,
}

/// A dedicated sub-bass oscillator tuned exactly 1 or 2 octaves below a base frequency.
pub struct SubOscillator {
    sample_rate: f32,
    pub frequency: f32,
    pub octave_offset: i32, // -1 or -2
    pub waveform: SubWaveform,
    phase: f32,
}

impl SubOscillator {
    pub fn new(sample_rate: f32, frequency: f32, octave_offset: i32, waveform: SubWaveform) -> Self {
        Self {
            sample_rate,
            frequency,
            octave_offset: octave_offset.clamp(-2, -1),
            waveform,
            phase: 0.0,
        }
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    pub fn set_octave_offset(&mut self, offset: i32) {
        self.octave_offset = offset.clamp(-2, -1);
    }
}

impl AudioNode for SubOscillator {
    fn process(&mut self) -> f32 {
        // Calculate octave-offset frequency: e.g. -1 octave is half frequency, -2 octaves is quarter
        let mult = match self.octave_offset {
            -2 => 0.25,
            _ => 0.5,
        };
        let sub_freq = self.frequency * mult;
        let dt = sub_freq / self.sample_rate;

        let sample = match self.waveform {
            SubWaveform::Sine => (self.phase * 2.0 * PI).sin(),
            SubWaveform::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            SubWaveform::Square => if self.phase < 0.5 { 1.0 } else { -1.0 },
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }
}
