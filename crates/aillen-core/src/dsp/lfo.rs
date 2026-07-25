use std::f32::consts::PI;

/// Waveforms supported by the reusable LFO.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LfoWaveform {
    Sine,
    Triangle,
    Saw,
    Square,
    RandomSampleHold,
}

/// A reusable Low-Frequency Oscillator (LFO) with Sample & Hold capability.
pub struct Lfo {
    sample_rate: f32,
    pub frequency: f32,
    pub waveform: LfoWaveform,
    phase: f32,
    
    // Random S&H states
    rng_state: u32,
    last_sh_val: f32,
    sh_timer: f32,
}

impl Lfo {
    pub fn new(sample_rate: f32, frequency: f32, waveform: LfoWaveform) -> Self {
        Self {
            sample_rate,
            frequency,
            waveform,
            phase: 0.0,
            rng_state: 987654321,
            last_sh_val: 0.0,
            sh_timer: 0.0,
        }
    }

    /// Linear congruential generator for S&H noise source
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.rng_state as f32) / (u32::MAX as f32)) * 2.0 - 1.0
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency.max(0.001);
    }

    /// Processes one sample frame and returns the LFO output value (nominally -1.0 to 1.0).
    pub fn process(&mut self) -> f32 {
        let dt = self.frequency / self.sample_rate;
        
        let out_val = match self.waveform {
            LfoWaveform::Sine => (self.phase * 2.0 * PI).sin(),
            LfoWaveform::Triangle => 1.0 - 4.0 * (self.phase - 0.5).abs(),
            LfoWaveform::Saw => 2.0 * self.phase - 1.0,
            LfoWaveform::Square => if self.phase < 0.5 { 1.0 } else { -1.0 },
            LfoWaveform::RandomSampleHold => {
                self.sh_timer += dt;
                if self.sh_timer >= 1.0 || self.sh_timer <= 0.0 {
                    self.sh_timer = 0.0;
                    self.last_sh_val = self.next_random();
                }
                self.last_sh_val
            }
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        out_val
    }
}
