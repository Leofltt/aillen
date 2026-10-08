use crate::synth::Voice;
use crate::dsp::oscillator::{Waveform, SubWaveform};
use crate::dsp::distortion::DistortionMode;
use super::{voice::SwaveVoice, SwaveMode, SwavePatch};

/// A polyphonic versatile SuperSaw / Ensemble synthesizer inspired by the Elektron Monomachine SuperWave.
pub struct SwaveSynth {
    voices: Vec<SwaveVoice>,
    pub master_patch: SwavePatch,
    pub realtime_update: bool,
    pub legato: bool,
    held_notes: Vec<f32>,
}

impl SwaveSynth {
    /// Creates a new `SwaveSynth` instance with the specified number of polyphonic voices.
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            voices.push(SwaveVoice::new(sample_rate));
        }
        Self {
            voices,
            master_patch: SwavePatch::default(),
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

    /// Sets operating mode (Saw vs Ensemble).
    pub fn set_mode(&mut self, mode: SwaveMode) {
        self.master_patch.mode = mode;
        self.update_voices();
    }

    /// Sets base oscillator waveform.
    pub fn set_base_waveform(&mut self, waveform: Waveform) {
        self.master_patch.base_waveform = waveform;
        self.update_voices();
    }

    /// Sets unison parameters: detune (UNIW), inner pair level (UNID), outer pair level (UNIX), and stereo spread.
    pub fn set_unison(&mut self, detune: f32, level: f32, ext_level: f32, stereo_spread: f32) {
        self.master_patch.unison_detune = detune;
        self.master_patch.unison_level = level;
        self.master_patch.unison_ext_level = ext_level;
        self.master_patch.stereo_spread = stereo_spread;
        self.update_voices();
    }

    /// Sets ensemble chord intervals and volume level.
    pub fn set_ensemble(&mut self, interval2: f32, interval3: f32, interval4: f32, level: f32) {
        self.master_patch.ensemble_intervals = [interval2, interval3, interval4];
        self.master_patch.ensemble_level = level;
        self.update_voices();
    }

    /// Sets sub-oscillator parameters (waveform, octave offset -1 or -2, gain level).
    pub fn set_sub_osc(&mut self, waveform: SubWaveform, octave: i32, gain: f32) {
        self.master_patch.sub_waveform = waveform;
        self.master_patch.sub_octave = octave;
        self.master_patch.sub_gain = gain;
        self.update_voices();
    }

    /// Configures the Monomachine Base & Width filter (base HP freq, width passband span, HP resonance, LP resonance, env depth).
    pub fn set_filter(&mut self, base: f32, width: f32, hp_q: f32, lp_q: f32, env_amount: f32) {
        self.master_patch.filter_base = base;
        self.master_patch.filter_width = width;
        self.master_patch.filter_hp_q = hp_q;
        self.master_patch.filter_lp_q = lp_q;
        self.master_patch.filter_env_amount = env_amount;
        self.update_voices();
    }

    /// Sets amplitude ADSR envelope.
    pub fn set_amp_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.amp_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets filter modulation ADSR envelope.
    pub fn set_filter_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.filter_adsr = [a, d, s, r];
        self.update_voices();
    }

    /// Sets overdrive distortion / saturation settings.
    pub fn set_drive(&mut self, mode: DistortionMode, gain: f32, mix: f32) {
        self.master_patch.drive_mode = mode;
        self.master_patch.drive_gain = gain;
        self.master_patch.drive_mix = mix;
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

    pub fn process_stereo(&mut self) -> (f32, f32) {
        let mut left_sum = 0.0;
        let mut right_sum = 0.0;
        for voice in &mut self.voices {
            if voice.active {
                let (l, r) = voice.process_stereo();
                left_sum += l;
                right_sum += r;
            }
        }
        let headroom = 1.0 / (self.voices.len() as f32).max(1.0).sqrt();
        (left_sum * headroom, right_sum * headroom)
    }
}

impl crate::dsp::AudioNode for SwaveSynth {
    fn process(&mut self) -> f32 {
        let (l, r) = self.process_stereo();
        (l + r) * 0.5
    }
}

impl crate::synth::PlayableInstrument for SwaveSynth {
    fn process(&mut self) -> (f32, f32) {
        self.process_stereo()
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
