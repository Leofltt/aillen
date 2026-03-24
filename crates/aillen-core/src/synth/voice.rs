use crate::dsp::{
    oscillator::{Oscillator, Waveform},
    filter::BiquadFilter,
    envelope::AdsrEnvelope,
    AudioNode, AudioProcessor,
};

pub struct SynthVoice {
    sample_rate: f32,
    osc1: Oscillator,
    osc2: Oscillator,
    filter: BiquadFilter,
    amp_env: AdsrEnvelope,
    filter_env: AdsrEnvelope,
    
    pub active: bool,
    pub base_frequency: f32,
    
    note_duration_samples: Option<usize>,
    samples_played: usize,
}

impl SynthVoice {
    pub fn new(sample_rate: f32) -> Self {
        let osc1 = Oscillator::new(sample_rate, 440.0, Waveform::Saw);
        let osc2 = Oscillator::new(sample_rate, 440.0, Waveform::Square);
        let filter = BiquadFilter::new_lowpass(sample_rate, 1000.0, 0.707);
        let amp_env = AdsrEnvelope::new(sample_rate, 0.01, 0.2, 0.5, 0.5);
        let filter_env = AdsrEnvelope::new(sample_rate, 0.05, 0.3, 0.2, 0.5);
        
        Self {
            sample_rate,
            osc1,
            osc2,
            filter,
            amp_env,
            filter_env,
            active: false,
            base_frequency: 440.0,
            note_duration_samples: None,
            samples_played: 0,
        }
    }
    
    pub fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
        self.osc1.set_frequency(frequency);
        self.osc2.set_frequency(frequency * 1.01);
    }
    
    pub fn note_on(&mut self, frequency: f32, _velocity: f32) {
        self.base_frequency = frequency;
        
        self.osc1.set_frequency(frequency);
        self.osc2.set_frequency(frequency * 1.01);
        
        self.amp_env.trigger_on();
        self.filter_env.trigger_on();
        self.active = true;
        
        self.note_duration_samples = None;
        self.samples_played = 0;
    }
    
    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            // Calculate how many samples need to pass before we trigger note off
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }
    
    pub fn note_off(&mut self) {
        self.amp_env.trigger_off();
        self.filter_env.trigger_off();
    }
    
    pub fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }
}

impl AudioNode for SynthVoice {
    fn process(&mut self) -> f32 {
        if !self.amp_env.is_active() {
            self.active = false;
            return 0.0;
        }
        
        // Handle automatic note off
        if let Some(target_samples) = self.note_duration_samples {
            if self.samples_played >= target_samples {
                self.note_off();
                self.note_duration_samples = None;
            } else {
                self.samples_played += 1;
            }
        }
        
        // Process envelopes
        let a_env = self.amp_env.process();
        let f_env = self.filter_env.process();
        
        // Modulate filter cutoff
        let cutoff = 100.0 + (4900.0 * f_env);
        self.filter.set_cutoff(cutoff);
        
        // Add oscillators
        let mut sample = 0.0;
        sample += self.osc1.process() * 0.5;
        sample += self.osc2.process() * 0.5;
        
        // Filter the signal
        sample = self.filter.process(sample);
        
        // Apply amplitude envelope
        sample * a_env
    }
}
