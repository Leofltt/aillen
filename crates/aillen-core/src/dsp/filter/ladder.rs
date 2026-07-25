use std::f32::consts::PI;
use crate::dsp::AudioProcessor;

/// A stable, zero-delay feedback (ZDF) 4-pole resonant ladder filter.
/// This matches the squelchy, liquid character of classic acid synths.
pub struct ResonantLadderFilter {
    sample_rate: f32,
    pub cutoff: f32,
    pub resonance: f32,
    s: [f32; 4],
    hp_state: f32,
}

impl ResonantLadderFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            cutoff: 1000.0,
            resonance: 0.5,
            s: [0.0; 4],
            hp_state: 0.0,
        }
    }

    pub fn set_params(&mut self, cutoff: f32, resonance: f32) {
        self.cutoff = cutoff;
        self.resonance = resonance;
    }
}

impl AudioProcessor for ResonantLadderFilter {
    fn process(&mut self, input: f32) -> f32 {
        let cutoff_hz = self.cutoff.clamp(20.0, self.sample_rate * 0.49);
        let f = (cutoff_hz * PI / self.sample_rate).tan();
        
        // Scale down resonance feedback at higher cutoffs to prevent piercing/harsh peaks
        let high_cutoff_damp = (1.0 - cutoff_hz / (self.sample_rate * 0.45)).clamp(0.2, 1.0);
        let r = 4.0 * self.resonance.clamp(0.0, 0.99) * high_cutoff_damp;

        let g = f / (1.0 + f);
        let g2 = g * g;
        let g3 = g2 * g;
        let g4 = g3 * g;

        let compensated_input = input * (1.0 + self.resonance * 3.0);
        let sigma = g3 * self.s[0] + g2 * self.s[1] + g * self.s[2] + self.s[3];
        let y4 = (g4 * compensated_input + sigma) / (1.0 + r * g4);

        // One-pole high-pass filter on the feedback signal to prevent low-end bass cancellation
        let hp_coeff = 0.025; // Cutoff around 175Hz at 44.1kHz
        self.hp_state += hp_coeff * (y4 - self.hp_state);
        let feedback_signal = y4 - self.hp_state;

        let u = compensated_input - r * feedback_signal;

        let mut val = u;
        for i in 0..4 {
            let v = (val - self.s[i]) * g;
            let y = v + self.s[i];
            self.s[i] = y + v;
            val = y;
        }

        // Prevent denormals in internal states and output
        for state in &mut self.s {
            if state.abs() < 1e-15 { *state = 0.0; }
        }
        if self.hp_state.abs() < 1e-15 { self.hp_state = 0.0; }
        
        if val.abs() < 1e-15 { 0.0 } else { val }
    }
}
