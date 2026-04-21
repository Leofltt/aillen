use crate::dsp::{AudioNode, oscillator::Waveform, filter::FilterType};
use crate::synth::Voice;
use super::{voice::TwoOpVoice, SynthMode, TwoOpPatch};

pub struct TwoOpSynth {
    voices: Vec<TwoOpVoice>,
    pub master_patch: TwoOpPatch,
    pub realtime_update: bool,
    pub legato: bool,
    held_notes: Vec<f32>, 
}

impl TwoOpSynth {
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            voices.push(TwoOpVoice::new(sample_rate));
        }
        Self {
            voices,
            master_patch: TwoOpPatch::default(),
            realtime_update: false,
            legato: false,
            held_notes: Vec::new(),
        }
    }
    
    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }

    pub fn set_realtime_update(&mut self, enabled: bool) {
        self.realtime_update = enabled;
    }

    fn update_voices(&mut self) {
        if self.realtime_update {
            let patch = self.master_patch;
            for voice in &mut self.voices {
                if voice.active {
                    voice.set_patch(patch);
                }
            }
        }
    }

    pub fn set_mode(&mut self, mode: SynthMode) {
        self.master_patch.mode = mode;
        self.update_voices();
    }

    pub fn set_osc1_waveform(&mut self, waveform: Waveform) {
        self.master_patch.osc1_waveform = waveform;
        self.update_voices();
    }

    pub fn set_osc2_waveform(&mut self, waveform: Waveform) {
        self.master_patch.osc2_waveform = waveform;
        self.update_voices();
    }

    pub fn set_osc1_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.osc1_adsr = [a, d, s, r];
        self.update_voices();
    }

    pub fn set_osc2_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.osc2_adsr = [a, d, s, r];
        self.update_voices();
    }

    pub fn set_filter_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.filter_adsr = [a, d, s, r];
        self.update_voices();
    }

    pub fn set_filter_params(&mut self, cutoff: f32, q: f32, filter_type: FilterType) {
        self.master_patch.filter_cutoff = cutoff;
        self.master_patch.filter_q = q;
        self.master_patch.filter_type = filter_type;
        self.update_voices();
    }

    pub fn set_filter_mod(&mut self, enabled: bool, amount: f32) {
        self.master_patch.filter_mod_enabled = enabled;
        self.master_patch.filter_env_amount = amount;
        self.update_voices();
    }

    pub fn set_modulation_params(&mut self, index: f32, ratio: f32, detune: f32) {
        self.master_patch.modulation_index = index;
        self.master_patch.osc2_ratio = ratio;
        self.master_patch.osc2_detune = detune;
        self.update_voices();
    }
    
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        if !self.held_notes.contains(&frequency) {
            self.held_notes.push(frequency);
        }
        
        let is_mono = self.voices.len() == 1;
        
        if is_mono && self.legato && self.voices[0].is_active() {
            self.voices[0].set_frequency(frequency);
        } else {
            let patch = self.master_patch;
            if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
                voice.set_patch(patch);
                voice.note_on(frequency, velocity);
            } else {
                self.voices[0].set_patch(patch);
                self.voices[0].note_on(frequency, velocity);
            }
        }
    }
    
    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        let patch = self.master_patch;
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
            voice.set_patch(patch);
            voice.trigger_note(frequency, velocity, duration_ms);
        } else {
            self.voices[0].set_patch(patch);
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

impl AudioNode for TwoOpSynth {
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
