use crate::dsp::{
    oscillator::PolyBlepOscillator,
    filter::BiquadFilter,
    envelope::AdsrEnvelope,
    AudioNode, AudioProcessor,
};
use crate::synth::Voice;
use super::{SynthMode, TwoOpPatch};

pub struct TwoOpVoice {
    sample_rate: f32,
    pub patch: TwoOpPatch,
    
    pub osc1: PolyBlepOscillator,
    pub osc2: PolyBlepOscillator,
    pub osc1_env: AdsrEnvelope,
    pub osc2_env: AdsrEnvelope,
    
    pub filter: BiquadFilter,
    pub filter_env: AdsrEnvelope,
    
    pub active: bool,
    pub base_frequency: f32,
    
    note_duration_samples: Option<usize>,
    samples_played: usize,
}

impl TwoOpVoice {
    pub fn new(sample_rate: f32) -> Self {
        let patch = TwoOpPatch::default();
        let osc1 = PolyBlepOscillator::new(sample_rate, 440.0, patch.osc1_waveform);
        let osc2 = PolyBlepOscillator::new(sample_rate, 440.0, patch.osc2_waveform);
        
        let osc1_env = AdsrEnvelope::new(sample_rate, patch.osc1_adsr[0], patch.osc1_adsr[1], patch.osc1_adsr[2], patch.osc1_adsr[3]);
        let osc2_env = AdsrEnvelope::new(sample_rate, patch.osc2_adsr[0], patch.osc2_adsr[1], patch.osc2_adsr[2], patch.osc2_adsr[3]);
        
        let filter = BiquadFilter::new(sample_rate, patch.filter_cutoff, patch.filter_q, patch.filter_type);
        let filter_env = AdsrEnvelope::new(sample_rate, patch.filter_adsr[0], patch.filter_adsr[1], patch.filter_adsr[2], patch.filter_adsr[3]);
        
        Self {
            sample_rate,
            patch,
            osc1,
            osc2,
            osc1_env,
            osc2_env,
            filter,
            filter_env,
            active: false,
            base_frequency: 440.0,
            note_duration_samples: None,
            samples_played: 0,
        }
    }

    pub fn set_patch(&mut self, patch: TwoOpPatch) {
        self.patch = patch;
        self.osc1.set_waveform(patch.osc1_waveform);
        self.osc2.set_waveform(patch.osc2_waveform);
        
        self.osc1_env.attack = patch.osc1_adsr[0];
        self.osc1_env.decay = patch.osc1_adsr[1];
        self.osc1_env.sustain = patch.osc1_adsr[2];
        self.osc1_env.release = patch.osc1_adsr[3];
        self.osc1_env.recalculate_rates();

        self.osc2_env.attack = patch.osc2_adsr[0];
        self.osc2_env.decay = patch.osc2_adsr[1];
        self.osc2_env.sustain = patch.osc2_adsr[2];
        self.osc2_env.release = patch.osc2_adsr[3];
        self.osc2_env.recalculate_rates();

        self.filter_env.attack = patch.filter_adsr[0];
        self.filter_env.decay = patch.filter_adsr[1];
        self.filter_env.sustain = patch.filter_adsr[2];
        self.filter_env.release = patch.filter_adsr[3];
        self.filter_env.recalculate_rates();

        self.filter.set_cutoff(patch.filter_cutoff);
        self.filter.set_q(patch.filter_q);
        self.filter.set_type(patch.filter_type);
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
        
        if self.patch.filter_mod_enabled {
            let cutoff = self.patch.filter_cutoff + (self.patch.filter_env_amount * f_env);
            self.filter.set_cutoff(cutoff);
        } else {
            self.filter.set_cutoff(self.patch.filter_cutoff);
        }
        
        let osc2_freq = self.base_frequency * self.patch.osc2_ratio + self.patch.osc2_detune;
        self.osc2.set_frequency(osc2_freq);

        let mut sample = 0.0;
        match self.patch.mode {
            SynthMode::Additive => {
                self.osc1.set_frequency(self.base_frequency);
                sample += self.osc1.process() * env1 * 0.5;
                sample += self.osc2.process() * env2 * 0.5;
            }
            SynthMode::Am => {
                self.osc1.set_frequency(self.base_frequency);
                let carrier = self.osc1.process();
                let modulator = self.osc2.process() * env2;
                sample = carrier * (1.0 + modulator * self.patch.modulation_index) * env1;
            }
            SynthMode::Rm => {
                self.osc1.set_frequency(self.base_frequency);
                let carrier = self.osc1.process();
                let modulator = self.osc2.process() * env2;
                sample = carrier * modulator * self.patch.modulation_index * env1;
            }
            SynthMode::Fm => {
                let modulator = self.osc2.process() * env2 * self.patch.modulation_index * self.base_frequency;
                self.osc1.set_frequency(self.base_frequency + modulator);
                sample = self.osc1.process() * env1;
            }
        }
        
        sample = self.filter.process(sample);
        sample
    }
}
