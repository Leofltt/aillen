use crate::dsp::AudioNode;

/// The curve shape mode of the envelope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnvelopeCurve {
    /// Linear ramp additions/subtractions.
    Linear,
    /// Analog RC filter model curve with punchy exponential decay and release.
    Exponential,
}

/// The phases of an ADSR envelope state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnvelopeState {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// An Attack-Decay-Sustain-Release (ADSR) envelope generator with versatile curve modes.
pub struct AdsrEnvelope {
    sample_rate: f32,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub state: EnvelopeState,
    pub curve: EnvelopeCurve,
    pub value: f32,
    attack_rate: f32,
    decay_rate: f32,
    release_rate: f32,
    // Exponential coefficients (1-pole smoothing factors)
    attack_coef: f32,
    decay_coef: f32,
    release_coef: f32,
}

impl AdsrEnvelope {
    pub fn new(sample_rate: f32, attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self::with_curve(sample_rate, attack, decay, sustain, release, EnvelopeCurve::Linear)
    }

    /// Creates a new `AdsrEnvelope` with explicit curve selection.
    pub fn with_curve(sample_rate: f32, attack: f32, decay: f32, sustain: f32, release: f32, curve: EnvelopeCurve) -> Self {
        let mut env = Self {
            sample_rate,
            attack, decay, sustain, release,
            state: EnvelopeState::Idle,
            curve,
            value: 0.0,
            attack_rate: 0.0,
            decay_rate: 0.0,
            release_rate: 0.0,
            attack_coef: 0.0,
            decay_coef: 0.0,
            release_coef: 0.0,
        };
        env.recalculate_rates();
        env
    }

    /// Sets the envelope curve mode (Linear vs Exponential).
    pub fn set_curve(&mut self, curve: EnvelopeCurve) {
        self.curve = curve;
        self.recalculate_rates();
    }
    
    pub fn recalculate_rates(&mut self) {
        // Linear rates
        self.attack_rate = if self.attack > 0.0 { 1.0 / (self.attack * self.sample_rate) } else { 1.0 };
        self.decay_rate = if self.decay > 0.0 { (1.0 - self.sustain) / (self.decay * self.sample_rate) } else { 1.0 };
        self.release_rate = if self.release > 0.0 { self.sustain.max(0.01) / (self.release * self.sample_rate) } else { 1.0 };

        // Exponential coefficients (attack reaches peak at duration; decay/release reaches ~99.3%: e^(-5) ≈ 0.0067)
        self.attack_coef = if self.attack > 0.0 { 1.0 - (-3.0 / (self.attack * self.sample_rate)).exp() } else { 1.0 };
        self.decay_coef = if self.decay > 0.0 { 1.0 - (-5.0 / (self.decay * self.sample_rate)).exp() } else { 1.0 };
        self.release_coef = if self.release > 0.0 { 1.0 - (-5.0 / (self.release * self.sample_rate)).exp() } else { 1.0 };
    }
    
    pub fn trigger_on(&mut self) {
        self.state = EnvelopeState::Attack;
    }
    
    pub fn trigger_off(&mut self) {
        if self.state != EnvelopeState::Idle {
            self.state = EnvelopeState::Release;
            self.release_rate = if self.release > 0.0 { self.value / (self.release * self.sample_rate) } else { 1.0 };
        }
    }
    
    pub fn is_active(&self) -> bool {
        self.state != EnvelopeState::Idle
    }
}

impl AudioNode for AdsrEnvelope {
    fn process(&mut self) -> f32 {
        match self.curve {
            EnvelopeCurve::Linear => {
                match self.state {
                    EnvelopeState::Idle => {
                        self.value = 0.0;
                    }
                    EnvelopeState::Attack => {
                        self.value += self.attack_rate;
                        if self.value >= 1.0 {
                            self.value = 1.0;
                            self.state = EnvelopeState::Decay;
                        }
                    }
                    EnvelopeState::Decay => {
                        self.value -= self.decay_rate;
                        if self.value <= self.sustain {
                            self.value = self.sustain;
                            self.state = EnvelopeState::Sustain;
                        }
                    }
                    EnvelopeState::Sustain => {
                        self.value = self.sustain;
                    }
                    EnvelopeState::Release => {
                        self.value -= self.release_rate;
                        if self.value <= 0.0 {
                            self.value = 0.0;
                            self.state = EnvelopeState::Idle;
                        }
                    }
                }
            }
            EnvelopeCurve::Exponential => {
                match self.state {
                    EnvelopeState::Idle => {
                        self.value = 0.0;
                    }
                    EnvelopeState::Attack => {
                        // Exponential rise toward virtual target 1.05 to reach 1.0 peak at specified duration
                        self.value += (1.05 - self.value) * self.attack_coef;
                        if self.value >= 1.0 {
                            self.value = 1.0;
                            self.state = EnvelopeState::Decay;
                        }
                    }
                    EnvelopeState::Decay => {
                        // Exponential decay towards sustain level
                        self.value += (self.sustain - self.value) * self.decay_coef;
                        if (self.value - self.sustain).abs() < 1e-3 || self.value <= self.sustain {
                            self.value = self.sustain;
                            self.state = EnvelopeState::Sustain;
                        }
                    }
                    EnvelopeState::Sustain => {
                        self.value = self.sustain;
                    }
                    EnvelopeState::Release => {
                        // Exponential fall towards virtual target -0.005 to cleanly reach 0.0 within finite time
                        self.value += (-0.005 - self.value) * self.release_coef;
                        if self.value <= 1e-4 {
                            self.value = 0.0;
                            self.state = EnvelopeState::Idle;
                        }
                    }
                }
            }
        }
        
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_envelope() {
        let mut env = AdsrEnvelope::new(100.0, 0.1, 0.1, 0.5, 0.1);
        assert_eq!(env.curve, EnvelopeCurve::Linear);
        env.trigger_on();
        for _ in 0..10 {
            env.process();
        }
        assert!(env.value >= 0.99); // Reached attack peak
    }

    #[test]
    fn test_exponential_envelope() {
        let mut env = AdsrEnvelope::with_curve(100.0, 0.1, 0.1, 0.5, 0.1, EnvelopeCurve::Exponential);
        assert_eq!(env.curve, EnvelopeCurve::Exponential);
        env.trigger_on();
        for _ in 0..10 {
            env.process();
        }
        assert!(env.value >= 0.95);
        // Test decay toward sustain
        for _ in 0..10 {
            env.process();
        }
        assert!((env.value - 0.5).abs() < 0.05);
        // Test release
        env.trigger_off();
        for _ in 0..15 {
            env.process();
        }
        assert_eq!(env.state, EnvelopeState::Idle);
        assert_eq!(env.value, 0.0);
    }
}
