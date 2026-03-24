use crate::dsp::AudioNode;
use super::voice::SynthVoice;

pub struct PolySynth {
    voices: Vec<SynthVoice>,
    pub legato: bool,
    held_notes: Vec<f32>, 
}

impl PolySynth {
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            voices.push(SynthVoice::new(sample_rate));
        }
        Self {
            voices,
            legato: false,
            held_notes: Vec::new(),
        }
    }
    
    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }
    
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        if !self.held_notes.contains(&frequency) {
            self.held_notes.push(frequency);
        }
        
        let is_mono = self.voices.len() == 1;
        
        if is_mono && self.legato && self.voices[0].is_active() {
            self.voices[0].set_frequency(frequency);
        } else {
            // Find a free voice
            if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
                voice.note_on(frequency, velocity);
            } else {
                // Steal the first voice
                self.voices[0].note_on(frequency, velocity);
            }
        }
    }
    
    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
            voice.trigger_note(frequency, velocity, duration_ms);
        } else {
            self.voices[0].trigger_note(frequency, velocity, duration_ms);
        }
    }
    
    pub fn note_off(&mut self, frequency: f32) {
        self.held_notes.retain(|&f| (f - frequency).abs() > 0.01);
        
        let is_mono = self.voices.len() == 1;
        
        if is_mono && self.legato {
            if let Some(&last_note) = self.held_notes.last() {
                self.voices[0].set_frequency(last_note);
                return;
            }
        }
        
        for voice in &mut self.voices {
            if (voice.base_frequency - frequency).abs() < 0.01 && voice.is_active() {
                voice.note_off();
            }
        }
    }
    
    pub fn note_off_all(&mut self) {
        self.held_notes.clear();
        for voice in &mut self.voices {
            voice.note_off();
        }
    }
}

impl AudioNode for PolySynth {
    fn process(&mut self) -> f32 {
        let mut mix = 0.0;
        for voice in &mut self.voices {
            if voice.active {
                mix += voice.process();
            }
        }
        let headroom = 1.0 / (self.voices.len() as f32).max(1.0).sqrt(); 
        mix * headroom
    }
}
