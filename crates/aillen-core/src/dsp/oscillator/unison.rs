use std::f32::consts::PI;
use crate::dsp::oscillator::Waveform;

/// A reusable Unison Engine stacking multiple detuned and panned oscillators.
pub struct UnisonEngine {
    sample_rate: f32,
    pub frequency: f32,
    pub waveform: Waveform,
    pub num_voices: usize,
    pub detune: f32,          // Detuning amount (0.0 to 0.1)
    pub stereo_spread: f32,   // Panning spread (0.0 to 1.0)
    phases: Vec<f32>,
    rng_state: u32,
}

impl UnisonEngine {
    pub fn new(sample_rate: f32, frequency: f32, num_voices: usize, waveform: Waveform) -> Self {
        let mut engine = Self {
            sample_rate,
            frequency,
            waveform,
            num_voices: num_voices.clamp(1, 7),
            detune: 0.03,
            stereo_spread: 0.8,
            phases: Vec::new(),
            rng_state: 42424242,
        } ;
        engine.reinit_phases();
        engine
    }

    /// LCG for initial phase randomization to avoid phase alignment cancellation
    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng_state as f32) / (u32::MAX as f32)
    }

    pub fn reinit_phases(&mut self) {
        self.phases = (0..self.num_voices).map(|_| self.next_random()).collect();
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
    }

    pub fn set_num_voices(&mut self, num_voices: usize) {
        let clamped = num_voices.clamp(1, 7);
        if self.num_voices != clamped {
            self.num_voices = clamped;
            self.reinit_phases();
        }
    }

    /// Helper PolyBLEP step correction for band-limited saw/square waves.
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

    /// Computes the next stereo sample frame of the unison stack, returning (Left, Right).
    pub fn process_stereo(&mut self) -> (f32, f32) {
        let mut left_sum = 0.0;
        let mut right_sum = 0.0;

        if self.num_voices == 0 {
            return (0.0, 0.0);
        }

        for i in 0..self.num_voices {
            // 1. Calculate detune frequency multiplier for this voice
            let detune_mult = if self.num_voices > 1 {
                let fraction = (i as f32) / ((self.num_voices - 1) as f32); // 0.0 to 1.0
                let offset = fraction * 2.0 - 1.0; // -1.0 to 1.0
                1.0 + offset * self.detune
            } else {
                1.0
            };

            let voice_freq = self.frequency * detune_mult;
            let dt = voice_freq / self.sample_rate;
            let t = self.phases[i];

            // 2. Generate band-limited wave
            let sample = match self.waveform {
                Waveform::Sine => (t * 2.0 * PI).sin(),
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
                Waveform::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
            };

            // Update phase
            self.phases[i] += dt;
            if self.phases[i] >= 1.0 {
                self.phases[i] -= 1.0;
            }

            // 3. Calculate panning for this voice
            let pan = if self.num_voices > 1 {
                let fraction = (i as f32) / ((self.num_voices - 1) as f32); // 0.0 to 1.0
                let centered_pan = fraction * 2.0 - 1.0; // -1.0 to 1.0
                centered_pan * self.stereo_spread
            } else {
                0.0
            };

            // Equal power panning
            let angle = (pan + 1.0) * (PI / 4.0);
            let pan_l = angle.cos();
            let pan_r = angle.sin();

            left_sum += sample * pan_l;
            right_sum += sample * pan_r;
        }

        // Normalize sum by square root of voice count to maintain constant perceived power
        let norm = 1.0 / (self.num_voices as f32).sqrt();
        (left_sum * norm, right_sum * norm)
    }
}
