use crate::dsp::oscillator::{SubWaveform, Waveform};
use crate::dsp::lfo::LfoWaveform;
use crate::dsp::distortion::DistortionMode;
use crate::synth::Voice;
use super::{voice::HubassVoice, SynthHubassPatch};
use std::f32::consts::PI;

/// A simple ring-buffered variable delay line with linear interpolation.
pub struct VariableDelay {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
}

impl VariableDelay {
    pub fn new(sample_rate: f32, max_delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay_samples],
            write_pos: 0,
            sample_rate,
        }
    }

    pub fn push(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
    }

    pub fn read(&self, delay_sec: f32) -> f32 {
        let delay_samples = (delay_sec * self.sample_rate).clamp(0.0, (self.buffer.len() - 2) as f32);
        let read_pos = (self.write_pos as f32 - delay_samples + self.buffer.len() as f32) % self.buffer.len() as f32;

        let idx0 = read_pos.floor() as usize % self.buffer.len();
        let idx1 = (idx0 + 1) % self.buffer.len();
        let frac = read_pos - read_pos.floor();

        self.buffer[idx0] * (1.0 - frac) + self.buffer[idx1] * frac
    }
}

/// A massive, versatile Rave & Bass Synthesizer with detuned unison, sub-bass, multi-mode filtering and chorus.
pub struct SynthHubass {
    voices: Vec<HubassVoice>,
    pub master_patch: SynthHubassPatch,
    pub realtime_update: bool,
    pub legato: bool,
    held_notes: Vec<f32>,

    sample_rate: f32,

    // Chorus Delay Lines (Stereo input, stereo output)
    delay_l: VariableDelay,
    delay_r: VariableDelay,

    // Chorus Jitter LFOs
    target_rate1: f32,
    target_rate2: f32,
    current_rate1: f32,
    current_rate2: f32,
    lfo_phase1: f32,
    lfo_phase2: f32,
    
    rand_samples_timer: usize,
    rand_samples_interval: usize,
    rng_state: u32,
}

impl SynthHubass {
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            voices.push(HubassVoice::new(sample_rate));
        }

        let delay_l = VariableDelay::new(sample_rate, 16384);
        let delay_r = VariableDelay::new(sample_rate, 16384);

        Self {
            voices,
            master_patch: SynthHubassPatch::default(),
            realtime_update: true,
            legato: true,
            held_notes: Vec::new(),
            sample_rate,
            delay_l,
            delay_r,
            target_rate1: 0.5,
            target_rate2: 0.8,
            current_rate1: 0.5,
            current_rate2: 0.8,
            lfo_phase1: 0.0,
            lfo_phase2: 0.5,
            rand_samples_timer: 0,
            rand_samples_interval: (sample_rate * 0.15) as usize,
            rng_state: 123456789,
        }
    }

    fn next_random(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng_state as f32) / (u32::MAX as f32)
    }

    pub fn set_legato(&mut self, legato: bool) {
        self.legato = legato;
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

    pub fn set_amp_adsr(&mut self, a: f32, d: f32, s: f32, r: f32) {
        self.master_patch.amp_adsr = [a, d, s, r];
        self.update_voices();
    }

    pub fn set_filter_params(&mut self, start_mult: f32, end_cf: f32, decay: f32, resonance: f32) {
        self.master_patch.filter_start_mult = start_mult;
        self.master_patch.filter_cutoff_end = end_cf;
        self.master_patch.filter_decay = decay;
        self.master_patch.filter_resonance = resonance;
        self.update_voices();
    }

    pub fn set_osc_unison(&mut self, waveform_idx: i32, detune: f32, spread: f32, num_voices: i32) {
        let wf = match waveform_idx {
            1 => Waveform::Square,
            2 => Waveform::Triangle,
            _ => Waveform::Saw,
        };
        self.master_patch.unison_waveform = wf;
        self.master_patch.unison_detune = detune.clamp(0.0, 0.2);
        self.master_patch.unison_spread = spread.clamp(0.0, 1.0);
        self.master_patch.unison_voices = num_voices.clamp(1, 7) as usize;
        self.update_voices();
    }

    pub fn set_osc_sub(&mut self, waveform_idx: i32, octave_offset: i32, gain: f32) {
        let wf = match waveform_idx {
            1 => SubWaveform::Triangle,
            2 => SubWaveform::Square,
            _ => SubWaveform::Sine,
        };
        self.master_patch.sub_waveform = wf;
        self.master_patch.sub_octave = octave_offset.clamp(-2, -1);
        self.master_patch.sub_gain = gain.clamp(0.0, 2.0);
        self.update_voices();
    }

    pub fn set_osc_noise(&mut self, gain: f32) {
        self.master_patch.noise_gain = gain.clamp(0.0, 1.0);
        self.update_voices();
    }

    pub fn set_filter_mode(&mut self, mode: i32) {
        self.master_patch.filter_mode = mode.clamp(0, 2);
        self.update_voices();
    }

    pub fn set_drive_mode(&mut self, mode_idx: i32, gain: f32, mix: f32) {
        self.master_patch.drive_mode = DistortionMode::from_i32(mode_idx);
        self.master_patch.drive_gain = gain.clamp(0.0, 10.0);
        self.master_patch.drive_mix = mix.clamp(0.0, 1.0);
        self.update_voices();
    }

    pub fn set_output_gain(&mut self, gain: f32) {
        self.master_patch.output_gain = gain.clamp(0.0, 5.0);
        self.update_voices();
    }

    pub fn set_lfo1(&mut self, waveform_idx: i32, speed_hz: f32, cutoff_depth: f32, pitch_depth: f32) {
        let wf = match waveform_idx {
            1 => LfoWaveform::Triangle,
            2 => LfoWaveform::Saw,
            3 => LfoWaveform::Square,
            4 => LfoWaveform::RandomSampleHold,
            _ => LfoWaveform::Sine,
        };
        self.master_patch.lfo1_waveform = wf;
        self.master_patch.lfo1_speed = speed_hz.max(0.001);
        self.master_patch.lfo1_cutoff_depth = cutoff_depth.clamp(0.0, 1.0);
        self.master_patch.lfo1_pitch_depth = pitch_depth.clamp(0.0, 1.0);
        self.update_voices();
    }

    pub fn set_chorus_params(&mut self, mix: f32, depth: f32) {
        self.master_patch.chorus_mix = mix;
        self.master_patch.chorus_depth = depth;
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

    pub fn note_off_all(&mut self) {
        self.held_notes.clear();
        for voice in &mut self.voices {
            voice.note_off();
        }
    }

    /// Process voice stereo outputs and run through built-in chorus delay.
    pub fn process_stereo(&mut self) -> (f32, f32) {
        let mut left_sum = 0.0;
        let mut right_sum = 0.0;
        let mut sub_sum = 0.0;
        
        for voice in &mut self.voices {
            if voice.active {
                let (vl, vr, vs) = voice.process_stereo();
                left_sum += vl;
                right_sum += vr;
                sub_sum += vs;
            }
        }
        
        let headroom = 1.0 / (self.voices.len() as f32).max(1.0).sqrt();
        let mono_l = left_sum * headroom;
        let mono_r = right_sum * headroom;
        let mono_sub = sub_sum * headroom;

        // 2. Modulate LFO rates (unirand + portamento emulation)
        self.rand_samples_timer += 1;
        if self.rand_samples_timer >= self.rand_samples_interval {
            self.rand_samples_timer = 0;
            self.target_rate1 = self.next_random() + 0.1;
            self.target_rate2 = self.next_random() + 0.1;
        }

        let port_coeff = 1.0 - (-1.0 / (0.01 * self.sample_rate)).exp();
        self.current_rate1 += (self.target_rate1 - self.current_rate1) * port_coeff;
        self.current_rate2 += (self.target_rate2 - self.current_rate2) * port_coeff;

        // 3. Process chorus LFOs
        self.lfo_phase1 += self.current_rate1 / self.sample_rate;
        if self.lfo_phase1 >= 1.0 {
            self.lfo_phase1 -= 1.0;
        }
        self.lfo_phase2 += self.current_rate2 / self.sample_rate;
        if self.lfo_phase2 >= 1.0 {
            self.lfo_phase2 -= 1.0;
        }

        let alfo = (self.lfo_phase1 * 2.0 * PI).sin() * 0.005 * self.master_patch.chorus_depth;
        let alfo2 = (self.lfo_phase2 * 2.0 * PI).sin() * 0.005 * self.master_patch.chorus_depth;

        let delay_sec1 = 0.1 + alfo;
        let delay_sec2 = 0.1 + alfo2;

        self.delay_l.push(mono_l);
        self.delay_r.push(mono_r);

        let adel = self.delay_l.read(delay_sec1);
        let adel2 = self.delay_r.read(delay_sec2);

        let mix = self.master_patch.chorus_mix;
        let left_mid_high = mono_l * (1.0 - mix) + adel * mix;
        let right_mid_high = mono_r * (1.0 - mix) + adel2 * mix;

        // Mix the clean mono sub-bass directly to both left/right outputs post-chorus
        let left = left_mid_high + mono_sub;
        let right = right_mid_high + mono_sub;

        (left, right)
    }
}

impl crate::synth::PlayableInstrument for SynthHubass {
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
