use crate::dsp::{AudioNode, oscillator::Waveform, filter::FilterType};
use crate::synth::Voice;
use super::{voice::TwoOpVoice, SynthMode, TwoOpPatch};

/// A polyphonic two-operator FM/AM/RM/Additive synthesizer.
pub struct TwoOpSynth {
    voices: Vec<TwoOpVoice>,
    /// Master patch parameters.
    pub master_patch: TwoOpPatch,
    /// When true, parameter tweaks immediately update currently ringing voices.
    pub realtime_update: bool,
    /// Legato mode (mono skipping of envelope triggers).
    pub legato: bool,
    held_notes: Vec<f32>, 
}

impl TwoOpSynth {
    /// Creates a new `TwoOpSynth` configured with `num_voices` polyphonic voices.
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
    
    /// Enables/disables Legato mode.
    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
    }

    /// Enables/disables real-time updating of active notes on parameter change.
    pub fn set_realtime_update(&mut self, enabled: bool) {
        self.realtime_update = enabled;
    }

    /// Syncs master patch changes to active voices if `realtime_update` is enabled.
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

    /// Sets the synthesis mode.
    pub fn set_mode(&mut self, mode: SynthMode) {
        self.master_patch.mode = mode;
        self.update_voices();
    }

    /// Sets Operator 1 (Carrier) waveform.
    pub fn set_osc1_waveform(&mut self, waveform: Waveform) {
        self.master_patch.osc1_waveform = waveform;
        self.update_voices();
    }

    /// Sets Operator 2 (Modulator) waveform.
    pub fn set_osc2_waveform(&mut self, waveform: Waveform) {
        self.master_patch.osc2_waveform = waveform;
        self.update_voices();
    }

    /// Sets Operator 1 ADSR envelope.
    pub fn set_osc1_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.osc1_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets Operator 2 ADSR envelope.
    pub fn set_osc2_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.osc2_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets Filter Cutoff ADSR envelope.
    pub fn set_filter_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.filter_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets active filter params (cutoff, Q, and filter type).
    pub fn set_filter_params(&mut self, cutoff: f32, q: f32, filter_type: FilterType) {
        self.master_patch.filter_cutoff = cutoff;
        self.master_patch.filter_q = q;
        self.master_patch.filter_type = filter_type;
        self.update_voices();
    }

    /// Sets filter modulation properties (enabled state and modulation depth).
    pub fn set_filter_mod(&mut self, enabled: bool, amount: f32) {
        self.master_patch.filter_mod_enabled = enabled;
        self.master_patch.filter_env_amount = amount;
        self.update_voices();
    }

    /// Sets modulator parameters (modulation index, frequency ratio, and detuning).
    pub fn set_modulation_params(&mut self, index: f32, ratio: f32, detune: f32) {
        self.master_patch.modulation_index = index;
        self.master_patch.osc2_ratio = ratio;
        self.master_patch.osc2_detune = detune;
        self.update_voices();
    }

    /// Sets self-feedback of Operator 2.
    pub fn set_osc2_feedback(&mut self, feedback: f32) {
        self.master_patch.osc2_feedback = feedback;
        self.update_voices();
    }

    /// Sets wavefolder settings (gain, mix).
    pub fn set_wavefold(&mut self, gain: f32, mix: f32) {
        self.master_patch.wavefold_gain = gain;
        self.master_patch.wavefold_mix = mix;
        self.update_voices();
    }

    /// Sets phase noise injection amounts for carrier and modulator.
    pub fn set_noise(&mut self, carrier_noise: f32, modulator_noise: f32) {
        self.master_patch.carrier_noise = carrier_noise;
        self.master_patch.modulator_noise = modulator_noise;
        self.update_voices();
    }

    /// Sets pitch sweep envelope (depth, decay).
    pub fn set_pitch_sweep(&mut self, depth: f32, decay: f32) {
        self.master_patch.pitch_sweep_depth = depth;
        self.master_patch.pitch_sweep_decay = decay;
        self.update_voices();
    }

    /// Sets voice LFO (waveform, speed, mod index depth, filter cutoff depth).
    pub fn set_lfo(&mut self, waveform: usize, speed: f32, mod_index: f32, cutoff: f32) {
        self.master_patch.lfo_waveform = waveform;
        self.master_patch.lfo_speed = speed;
        self.master_patch.lfo_mod_index = mod_index;
        self.master_patch.lfo_cutoff = cutoff;
        self.update_voices();
    }
    
    /// Triggers note playback.
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
    
    /// Plays a timed note (triggers on immediately, note-off after `duration_ms`).
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
    
    /// Releases notes matching the specified target frequency.
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
    
    /// Silences all active voices immediately.
    pub fn note_off_all(&mut self) {
        self.held_notes.clear();
        for voice in &mut self.voices {
            voice.note_off();
        }
    }
}

impl AudioNode for TwoOpSynth {
    /// Generates a single sample frame, summing outputs of active voices with headroom scaling.
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

impl crate::synth::PlayableInstrument for TwoOpSynth {
    fn process(&mut self) -> (f32, f32) {
        let val = <Self as AudioNode>::process(self);
        (val, val)
    }

    fn note_on(&mut self, frequency: f32, velocity: f32) {
        self.note_on(frequency, velocity);
    }

    fn note_off(&mut self, frequency: f32) {
        self.note_off(frequency);
    }

    fn note_off_all(&mut self) {
        self.note_off_all();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
