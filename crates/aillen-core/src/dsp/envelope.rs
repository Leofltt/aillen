use super::AudioNode;

/// The phases of an ADSR envelope state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EnvelopeState {
    /// Idle phase (value is 0.0).
    Idle,
    /// Attack phase (fading from 0.0 to 1.0).
    Attack,
    /// Decay phase (fading from 1.0 to the sustain level).
    Decay,
    /// Sustain phase (holding value at the sustain level).
    Sustain,
    /// Release phase (fading from the current value down to 0.0).
    Release,
}

/// An Attack-Decay-Sustain-Release (ADSR) envelope generator.
/// Used to modulate volume, cutoff, or pitch parameters over time.
pub struct AdsrEnvelope {
    sample_rate: f32,
    
    /// Attack time in seconds (fade in).
    pub attack: f32,
    /// Decay time in seconds (fade to sustain level).
    pub decay: f32,
    /// Sustain level amplitude (range 0.0 to 1.0).
    pub sustain: f32,
    /// Release time in seconds (fade out).
    pub release: f32,
    
    // Internal state
    state: EnvelopeState,
    value: f32,
    
    attack_rate: f32,
    decay_rate: f32,
    release_rate: f32,
}

impl AdsrEnvelope {
    /// Creates a new `AdsrEnvelope` configured for the target sample rate.
    pub fn new(sample_rate: f32, attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        let mut env = Self {
            sample_rate,
            attack, decay, sustain, release,
            state: EnvelopeState::Idle,
            value: 0.0,
            attack_rate: 0.0,
            decay_rate: 0.0,
            release_rate: 0.0,
        };
        env.recalculate_rates();
        env
    }
    
    /// Recalculates internally cached rate increments based on parameters and sample rate.
    pub fn recalculate_rates(&mut self) {
        self.attack_rate = if self.attack > 0.0 { 1.0 / (self.attack * self.sample_rate) } else { 1.0 };
        self.decay_rate = if self.decay > 0.0 { (1.0 - self.sustain) / (self.decay * self.sample_rate) } else { 1.0 };
        self.release_rate = if self.release > 0.0 { self.sustain / (self.release * self.sample_rate) } else { 1.0 };
    }
    
    /// Triggers the attack phase of the envelope.
    pub fn trigger_on(&mut self) {
        self.state = EnvelopeState::Attack;
    }
    
    /// Triggers the release phase of the envelope.
    pub fn trigger_off(&mut self) {
        if self.state != EnvelopeState::Idle {
            self.state = EnvelopeState::Release;
            self.release_rate = if self.release > 0.0 { self.value / (self.release * self.sample_rate) } else { 1.0 };
        }
    }
    
    /// Checks if the envelope is active (in any state other than Idle).
    pub fn is_active(&self) -> bool {
        self.state != EnvelopeState::Idle
    }
}

impl AudioNode for AdsrEnvelope {
    /// Progresses the envelope state by one sample and returns the current amplitude value.
    fn process(&mut self) -> f32 {
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
        
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adsr_envelope() {
        let mut env = AdsrEnvelope::new(44100.0, 0.01, 0.01, 0.5, 0.01);
        assert!(!env.is_active());
        
        env.trigger_on();
        assert!(env.is_active());
        
        let sample_attack = env.process();
        assert!(sample_attack > 0.0);
        
        env.trigger_off();
        let sample_release = env.process();
        assert!(sample_release < sample_attack);
    }
}
