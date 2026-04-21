use super::{SynthMode, voice::TwoOpVoice};
use crate::dsp::{AudioNode, filter::FilterType, oscillator::Waveform};
use crate::synth::Voice;

pub struct TwoOpSynth {
    voices: Vec<TwoOpVoice>,
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
            legato: false,
            held_notes: Vec::new(),
        }
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }

    pub fn set_mode(&mut self, mode: SynthMode) {
        for voice in &mut self.voices {
            voice.mode = mode;
        }
    }

    pub fn set_osc1_waveform(&mut self, waveform: Waveform) {
        for voice in &mut self.voices {
            voice.osc1.set_waveform(waveform);
        }
    }

    pub fn set_osc2_waveform(&mut self, waveform: Waveform) {
        for voice in &mut self.voices {
            voice.osc2.set_waveform(waveform);
        }
    }

    pub fn set_osc1_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        for voice in &mut self.voices {
            voice.osc1_env.attack = a;
            voice.osc1_env.decay = d;
            voice.osc1_env.sustain = s;
            voice.osc1_env.release = r;
            voice.osc1_env.recalculate_rates();
        }
    }

    pub fn set_osc2_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        for voice in &mut self.voices {
            voice.osc2_env.attack = a;
            voice.osc2_env.decay = d;
            voice.osc2_env.sustain = s;
            voice.osc2_env.release = r;
            voice.osc2_env.recalculate_rates();
        }
    }

    pub fn set_filter_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        for voice in &mut self.voices {
            voice.filter_env.attack = a;
            voice.filter_env.decay = d;
            voice.filter_env.sustain = s;
            voice.filter_env.release = r;
            voice.filter_env.recalculate_rates();
        }
    }

    pub fn set_filter_params(&mut self, cutoff: f32, q: f32, filter_type: FilterType) {
        for voice in &mut self.voices {
            voice.filter.set_cutoff(cutoff);
            voice.filter.set_q(q);
            voice.filter.set_type(filter_type);
        }
    }

    pub fn set_filter_mod(&mut self, enabled: bool, amount: f32) {
        for voice in &mut self.voices {
            voice.filter_mod_enabled = enabled;
            voice.filter_env_amount = amount;
        }
    }

    pub fn set_modulation_params(&mut self, index: f32, ratio: f32, detune: f32) {
        for voice in &mut self.voices {
            voice.modulation_index = index;
            voice.osc2_ratio = ratio;
            voice.osc2_detune = detune;
        }
    }

    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        if !self.held_notes.contains(&frequency) {
            self.held_notes.push(frequency);
        }

        let is_mono = self.voices.len() == 1;

        if is_mono && self.legato && self.voices[0].is_active() {
            self.voices[0].set_frequency(frequency);
        } else {
            if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
                voice.note_on(frequency, velocity);
            } else {
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
