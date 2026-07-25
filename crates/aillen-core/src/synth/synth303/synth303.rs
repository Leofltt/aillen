use crate::dsp::{AudioNode, oscillator::Waveform};
use crate::synth::Voice;
use super::{voice::Synth303Voice, Synth303Patch};

/// A monophonic/polyphonic 303-like bass/lead synthesizer.
pub struct Synth303 {
    voices: Vec<Synth303Voice>,
    /// Master patch parameters.
    pub master_patch: Synth303Patch,
    /// When true, parameter tweaks immediately update currently ringing voices.
    pub realtime_update: bool,
    /// Legato mode (mono skipping of envelope triggers, glides pitch).
    pub legato: bool,
    held_notes: Vec<f32>,
}

impl Synth303 {
    /// Creates a new `Synth303` configured with `num_voices` voices.
    /// By default, we use 1 voice for monophonic/legato 303 behavior.
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            voices.push(Synth303Voice::new(sample_rate));
        }
        Self {
            voices,
            master_patch: Synth303Patch::default(),
            realtime_update: true, // 303 is heavily tweaked in real-time
            legato: true,          // Default to legato on for TB-303 behavior
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

    /// Sets the waveform.
    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.master_patch.waveform = waveform;
        self.update_voices();
    }

    /// Sets Amplitude envelope.
    pub fn set_amp_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.amp_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets Filter Cutoff envelope.
    pub fn set_filter_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.filter_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets Pitch envelope.
    pub fn set_pitch_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.pitch_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets active filter parameters.
    pub fn set_filter_params(&mut self, cutoff: f32, resonance: f32) {
        self.master_patch.filter_cutoff = cutoff;
        self.master_patch.filter_resonance = resonance;
        self.update_voices();
    }

    /// Sets filter modulation depth.
    pub fn set_filter_mod(&mut self, amount: f32) {
        self.master_patch.filter_env_amount = amount;
        self.update_voices();
    }

    /// Sets pitch modulation depth.
    pub fn set_pitch_mod(&mut self, amount: f32) {
        self.master_patch.pitch_env_amount = amount;
        self.update_voices();
    }

    /// Sets oscillator pulse width and PWM parameters.
    pub fn set_pwm_params(&mut self, pw: f32, rate: f32, depth: f32) {
        self.master_patch.pulse_width = pw;
        self.master_patch.pwm_rate = rate;
        self.master_patch.pwm_depth = depth;
        self.update_voices();
    }

    /// Sets Glide/Portamento time in seconds.
    pub fn set_glide_time(&mut self, seconds: f32) {
        self.master_patch.glide_time = seconds;
        self.update_voices();
    }

    /// Triggers note playback.
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        if !self.held_notes.contains(&frequency) {
            self.held_notes.push(frequency);
        }

        let is_mono = self.voices.len() == 1;

        if is_mono && self.legato && self.voices[0].is_active() {
            // Legato behavior: slide to new pitch, do not retrigger envelopes
            self.voices[0].set_frequency(frequency);
        } else {
            // Standard/Polyphonic trigger
            let patch = self.master_patch;
            if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
                voice.set_patch(patch);
                voice.note_on(frequency, velocity);
            } else {
                // Steal first voice
                self.voices[0].set_patch(patch);
                self.voices[0].note_on(frequency, velocity);
            }
        }
    }

    /// Plays a timed note.
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
        if frequency <= 0.0 {
            self.note_off_all();
            return;
        }

        self.held_notes.retain(|&f| (f - frequency).abs() > 0.01);

        let is_mono = self.voices.len() == 1;
        if is_mono {
            if let Some(&last_note) = self.held_notes.last() {
                if self.legato {
                    self.voices[0].set_frequency(last_note);
                    return;
                }
            } else {
                // No more notes held on this monophonic track, turn off the voice
                self.voices[0].note_off();
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

impl AudioNode for Synth303 {
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

impl crate::synth::PlayableInstrument for Synth303 {
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
