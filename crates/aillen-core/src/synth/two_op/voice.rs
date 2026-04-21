use crate::dsp::{
    oscillator::{PolyBlepOscillator, Waveform},
    filter::BiquadFilter,
    envelope::AdsrEnvelope,
    AudioNode, AudioProcessor,
};
use crate::synth::Voice;
use super::SynthMode;

pub struct TwoOpVoice {
    sample_rate: f32,
    pub mode: SynthMode,
    
    pub osc1: PolyBlepOscillator,
    pub osc2: PolyBlepOscillator,
    pub osc1_env: AdsrEnvelope,
    pub osc2_env: AdsrEnvelope,
    
    pub filter: BiquadFilter,
    pub filter_env: AdsrEnvelope,
    pub filter_mod_enabled: bool,
    pub filter_env_amount: f32,
    
    pub modulation_index: f32,
    pub osc2_detune: f32,
    pub osc2_ratio: f32,
    
    pub active: bool,
    pub base_frequency: f32,
    
    note_duration_samples: Option<usize>,
    samples_played: usize,
}

impl TwoOpVoice {
    pub fn new(sample_rate: f32) -> Self {
        let osc1 = PolyBlepOscillator::new(sample_rate, 440.0, Waveform::Saw);
        let osc2 = PolyBlepOscillator::new(sample_rate, 440.0, Waveform::Saw);
        let osc1_env = AdsrEnvelope::new(sample_rate, 0.01, 0.2, 0.5, 0.5);
        let osc2_env = AdsrEnvelope::new(sample_rate, 0.01, 0.2, 0.5, 0.5);
        
        let filter = BiquadFilter::new_lowpass(sample_rate, 1000.0, 0.707);
        let filter_env = AdsrEnvelope::new(sample_rate, 0.05, 0.3, 0.2, 0.5);
        
        Self {
            sample_rate,
            mode: SynthMode::Additive,
            osc1,
            osc2,
            osc1_env,
            osc2_env,
            filter,
            filter_env,
            filter_mod_enabled: true,
            filter_env_amount: 5000.0,
            modulation_index: 1.0,
            osc2_detune: 0.0,
            osc2_ratio: 1.0,
            active: false,
            base_frequency: 440.0,
            note_duration_samples: None,
            samples_played: 0,
        }
    }
    
    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }
}

impl Voice for TwoOpVoice {
    fn note_on(&mut self, frequency: f32, _velocity: f32) {
        self.base_frequency = frequency;
        self.osc1_env.trigger_on();
        self.osc2_env.trigger_on();
        self.filter_env.trigger_on();
        self.active = true;
        self.note_duration_samples = None;
        self.samples_played = 0;
    }

    fn note_off(&mut self) {
        self.osc1_env.trigger_off();
        self.osc2_env.trigger_off();
        self.filter_env.trigger_off();
    }

    fn is_active(&self) -> bool {
        self.osc1_env.is_active() || self.osc2_env.is_active()
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
    }
}

impl AudioNode for TwoOpVoice {
    fn process(&mut self) -> f32 {
        if !self.is_active() {
            self.active = false;
            return 0.0;
        }
        
        if let Some(target_samples) = self.note_duration_samples {
            if self.samples_played >= target_samples {
                self.note_off();
                self.note_duration_samples = None;
            } else {
                self.samples_played += 1;
            }
        }
        
        let env1 = self.osc1_env.process();
        let env2 = self.osc2_env.process();
        let f_env = self.filter_env.process();
        
        if self.filter_mod_enabled {
            let cutoff = self.filter.cutoff + (self.filter_env_amount * f_env);
            self.filter.set_cutoff(cutoff);
        }
        
        let osc2_freq = self.base_frequency * self.osc2_ratio + self.osc2_detune;
        self.osc2.set_frequency(osc2_freq);

        let mut sample = 0.0;
        match self.mode {
            SynthMode::Additive => {
                self.osc1.set_frequency(self.base_frequency);
                sample += self.osc1.process() * env1 * 0.5;
                sample += self.osc2.process() * env2 * 0.5;
            }
            SynthMode::Am => {
                self.osc1.set_frequency(self.base_frequency);
                let carrier = self.osc1.process();
                let modulator = self.osc2.process() * env2;
                sample = carrier * (1.0 + modulator * self.modulation_index) * env1;
            }
            SynthMode::Rm => {
                self.osc1.set_frequency(self.base_frequency);
                let carrier = self.osc1.process();
                let modulator = self.osc2.process() * env2;
                sample = carrier * modulator * self.modulation_index * env1;
            }
            SynthMode::Fm => {
                let modulator = self.osc2.process() * env2 * self.modulation_index * self.base_frequency;
                self.osc1.set_frequency(self.base_frequency + modulator);
                sample = self.osc1.process() * env1;
            }
        }
        
        sample = self.filter.process(sample);
        sample
    }
}
