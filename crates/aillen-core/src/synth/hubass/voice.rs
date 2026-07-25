use crate::dsp::{
    envelope::{AdsrEnvelope, ExponEnvelope},
    filter::biquad::{BiquadFilter, FilterType},
    filter::ladder::ResonantLadderFilter,
    filter::formant::FormantFilter,
    oscillator::{UnisonEngine, SubOscillator},
    lfo::{Lfo},
    distortion::{Distortion},
    AudioNode, AudioProcessor,
};
use crate::synth::Voice;
use super::SynthHubassPatch;

/// Upgraded stereo Hubass voice supporting unison, sub-bass, multi-mode filtering, and drive.
pub struct HubassVoice {
    sample_rate: f32,
    pub patch: SynthHubassPatch,

    pub unison: UnisonEngine,
    pub sub: SubOscillator,
    
    // Noise Generator
    rng_state: u32,

    // Envelopes
    pub amp_env: AdsrEnvelope,
    pub filter_env: ExponEnvelope,

    // Modular LFO 1
    pub lfo1: Lfo,

    // Parallel Stereo Filters
    pub filter_lp_l: ResonantLadderFilter,
    pub filter_lp_r: ResonantLadderFilter,
    pub filter_bp_l: BiquadFilter,
    pub filter_bp_r: BiquadFilter,
    pub filter_formant_l: FormantFilter,
    pub filter_formant_r: FormantFilter,

    // Parallel Stereo Distortion/Drive
    pub dist_l: Distortion,
    pub dist_r: Distortion,

    pub active: bool,
    pub base_frequency: f32,
    pub current_frequency: f32,

    note_duration_samples: Option<usize>,
    samples_played: usize,
}

impl HubassVoice {
    pub fn new(sample_rate: f32) -> Self {
        let patch = SynthHubassPatch::default();
        
        let unison = UnisonEngine::new(sample_rate, 220.0, patch.unison_voices, patch.unison_waveform);
        let sub = SubOscillator::new(sample_rate, 220.0, patch.sub_octave, patch.sub_waveform);
        
        let amp_env = AdsrEnvelope::new(sample_rate, patch.amp_adsr[0], patch.amp_adsr[1], patch.amp_adsr[2], patch.amp_adsr[3]);
        let filter_env = ExponEnvelope::new(sample_rate);
        
        let lfo1 = Lfo::new(sample_rate, patch.lfo1_speed, patch.lfo1_waveform);

        let filter_lp_l = ResonantLadderFilter::new(sample_rate);
        let filter_lp_r = ResonantLadderFilter::new(sample_rate);
        let filter_bp_l = BiquadFilter::new(sample_rate, 800.0, 1.0, FilterType::BandPass);
        let filter_bp_r = BiquadFilter::new(sample_rate, 800.0, 1.0, FilterType::BandPass);
        let filter_formant_l = FormantFilter::new(sample_rate);
        let filter_formant_r = FormantFilter::new(sample_rate);

        let dist_l = Distortion::new(patch.drive_mode, patch.drive_gain, patch.drive_mix);
        let dist_r = Distortion::new(patch.drive_mode, patch.drive_gain, patch.drive_mix);

        Self {
            sample_rate,
            patch,
            unison,
            sub,
            rng_state: 88888888,
            amp_env,
            filter_env,
            lfo1,
            filter_lp_l,
            filter_lp_r,
            filter_bp_l,
            filter_bp_r,
            filter_formant_l,
            filter_formant_r,
            dist_l,
            dist_r,
            active: false,
            base_frequency: 220.0,
            current_frequency: 220.0,
            note_duration_samples: None,
            samples_played: 0,
        }
    }

    fn next_noise(&mut self) -> f32 {
        self.rng_state = self.rng_state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.rng_state as f32) / (u32::MAX as f32)) * 2.0 - 1.0
    }

    pub fn set_patch(&mut self, patch: SynthHubassPatch) {
        self.patch = patch;
        
        self.amp_env.attack = patch.amp_adsr[0];
        self.amp_env.decay = patch.amp_adsr[1];
        self.amp_env.sustain = patch.amp_adsr[2];
        self.amp_env.release = patch.amp_adsr[3];
        self.amp_env.recalculate_rates();

        self.unison.waveform = patch.unison_waveform;
        self.unison.set_num_voices(patch.unison_voices);
        self.unison.detune = patch.unison_detune;
        self.unison.stereo_spread = patch.unison_spread;

        self.sub.waveform = patch.sub_waveform;
        self.sub.set_octave_offset(patch.sub_octave);

        self.lfo1.waveform = patch.lfo1_waveform;
        self.lfo1.set_frequency(patch.lfo1_speed);

        self.dist_l.mode = patch.drive_mode;
        self.dist_l.drive = patch.drive_gain;
        self.dist_l.mix = patch.drive_mix;

        self.dist_r.mode = patch.drive_mode;
        self.dist_r.drive = patch.drive_gain;
        self.dist_r.mix = patch.drive_mix;
    }

    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }

    /// Processes one sample frame and returns (Left Mid-High, Right Mid-High, Mono Sub).
    pub fn process_stereo(&mut self) -> (f32, f32, f32) {
        if !self.is_active() {
            self.active = false;
            return (0.0, 0.0, 0.0);
        }

        if let Some(target_samples) = self.note_duration_samples {
            if self.samples_played >= target_samples {
                self.note_off();
                self.note_duration_samples = None;
            } else {
                self.samples_played += 1;
            }
        }

        // 1. Process LFO 1
        let lfo1_val = self.lfo1.process();

        // Fast pitch slide / glide (Portamento) for hoover sweep feel
        let glide_coeff = 1.0 - (-1.0 / (0.05 * self.sample_rate)).exp();
        self.current_frequency += (self.base_frequency - self.current_frequency) * glide_coeff;

        // Apply LFO Pitch Modulation
        let pitch_mod = self.patch.lfo1_pitch_depth * lfo1_val * 20.0;
        let final_freq = (self.current_frequency + pitch_mod).max(20.0);

        // Update oscillators
        self.unison.set_frequency(final_freq);
        self.sub.set_frequency(final_freq);

        // 2. Generate Oscillator Signals
        let (unison_l, unison_r) = self.unison.process_stereo();
        let sub_val = self.sub.process();
        let noise_val = self.next_noise();

        // Mixer Stage (routing unison and noise to filter)
        let osc_l = unison_l * self.patch.unison_gain 
            + noise_val * self.patch.noise_gain;
        let osc_r = unison_r * self.patch.unison_gain 
            + noise_val * self.patch.noise_gain;

        // 3. Process Envelopes & Cutoff
        let env_cutoff = self.filter_env.process();
        
        // Apply LFO Cutoff Modulation
        let lfo_cutoff_mod = self.patch.lfo1_cutoff_depth * lfo1_val * 1000.0;
        let final_cutoff = (env_cutoff + lfo_cutoff_mod).max(20.0);

        // 4. Stereo Filtering
        let (filtered_l, filtered_r) = match self.patch.filter_mode {
            1 => {
                self.filter_bp_l.set_cutoff(final_cutoff);
                self.filter_bp_l.set_q(self.patch.filter_resonance * 10.0 + 0.5);
                self.filter_bp_r.set_cutoff(final_cutoff);
                self.filter_bp_r.set_q(self.patch.filter_resonance * 10.0 + 0.5);

                (self.filter_bp_l.process(osc_l), self.filter_bp_r.process(osc_r))
            }
            2 => {
                let vowel_morph = ((final_cutoff - 100.0) / 1500.0).clamp(0.0, 1.0);
                self.filter_formant_l.set_vowel(vowel_morph);
                self.filter_formant_r.set_vowel(vowel_morph);

                (self.filter_formant_l.process(osc_l), self.filter_formant_r.process(osc_r))
            }
            _ => {
                self.filter_lp_l.set_params(final_cutoff, self.patch.filter_resonance);
                self.filter_lp_r.set_params(final_cutoff, self.patch.filter_resonance);

                (self.filter_lp_l.process(osc_l), self.filter_lp_r.process(osc_r))
            }
        };

        // 5. Stereo Saturation / Distortion Drive
        let driven_l = self.dist_l.process(filtered_l);
        let driven_r = self.dist_r.process(filtered_r);

        // 6. Amplitude Envelope with user-configurable master output gain
        let amp_val = self.amp_env.process();
        let gain = self.patch.output_gain;

        let mid_high_l = driven_l * amp_val * gain;
        let mid_high_r = driven_r * amp_val * gain;
        let clean_sub = sub_val * self.patch.sub_gain * amp_val * gain;

        (mid_high_l, mid_high_r, clean_sub)
    }
}

impl Voice for HubassVoice {
    fn note_on(&mut self, frequency: f32, _velocity: f32) {
        self.base_frequency = frequency;
        if !self.active {
            self.current_frequency = frequency;
            self.active = true;
        }
        self.amp_env.trigger_on();
        
        let start_cf = frequency * self.patch.filter_start_mult;
        let end_cf = self.patch.filter_cutoff_end;
        self.filter_env.trigger(start_cf, end_cf, self.patch.filter_decay);
        
        self.note_duration_samples = None;
        self.samples_played = 0;
    }

    fn note_off(&mut self) {
        self.amp_env.trigger_off();
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
    }
}

impl AudioNode for HubassVoice {
    fn process(&mut self) -> f32 {
        let (l, _, sub) = self.process_stereo();
        l + sub
    }
}
