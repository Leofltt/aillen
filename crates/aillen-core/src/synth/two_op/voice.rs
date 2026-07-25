use crate::dsp::{
    filter::BiquadFilter,
    envelope::{AdsrEnvelope, ExponEnvelope},
    lfo::{Lfo, LfoWaveform},
    oscillator::Waveform,
    AudioNode, AudioProcessor,
};
use crate::synth::Voice;
use super::{SynthMode, TwoOpPatch};

/// Helper to evaluate naive waveforms at a given normalized phase [0.0, 1.0]
fn eval_waveform(waveform: Waveform, phase: f32) -> f32 {
    let t = phase.rem_euclid(1.0);
    match waveform {
        Waveform::Sine => (t * 2.0 * std::f32::consts::PI).sin(),
        Waveform::Saw => 2.0 * t - 1.0,
        Waveform::Square => if t < 0.5 { 1.0 } else { -1.0 },
        Waveform::Triangle => 1.0 - 4.0 * (t - 0.5).abs(),
    }
}

/// Helper to apply diode-reflection wavefolding
fn wavefold(input: f32, gain: f32, mix: f32) -> f32 {
    let x = input * gain;
    let mut v = x;
    for _ in 0..3 {
        if v > 1.0 {
            v = 2.0 - v;
        } else if v < -1.0 {
            v = -2.0 - v;
        } else {
            break;
        }
    }
    input + (v - input) * mix
}

/// A single polyphonic voice for the `TwoOpSynth` with Phase Modulation, Modulator Feedback, Wavefolding, and Pitch Sweeps.
pub struct TwoOpVoice {
    sample_rate: f32,
    /// Active patch parameters for this voice.
    pub patch: TwoOpPatch,
    
    // Phase accumulators
    pub osc1_phase: f32,
    pub osc2_phase: f32,
    pub osc2_prev_out: f32,
    
    /// Amplitude envelope for Operator 1.
    pub osc1_env: AdsrEnvelope,
    /// Amplitude/modulation envelope for Operator 2.
    pub osc2_env: AdsrEnvelope,
    
    /// Low/high-pass biquad filter.
    pub filter: BiquadFilter,
    /// Filter cutoff modulation envelope.
    pub filter_env: AdsrEnvelope,
    
    /// Pitch sweep decay envelope.
    pub pitch_env: ExponEnvelope,
    /// Voice LFO.
    pub lfo: Lfo,
    
    /// Whether this voice is active and producing sound.
    pub active: bool,
    /// The fundamental midi frequency of the note triggered.
    pub base_frequency: f32,
    
    note_duration_samples: Option<usize>,
    samples_played: usize,
    rng_seed: u32,
}

impl TwoOpVoice {
    /// Initializes a new `TwoOpVoice` configured for the target sample rate.
    pub fn new(sample_rate: f32) -> Self {
        let patch = TwoOpPatch::default();
        
        let osc1_env = AdsrEnvelope::new(sample_rate, patch.osc1_adsr[0], patch.osc1_adsr[1], patch.osc1_adsr[2], patch.osc1_adsr[3]);
        let osc2_env = AdsrEnvelope::new(sample_rate, patch.osc2_adsr[0], patch.osc2_adsr[1], patch.osc2_adsr[2], patch.osc2_adsr[3]);
        
        let filter = BiquadFilter::new(sample_rate, patch.filter_cutoff, patch.filter_q, patch.filter_type);
        let filter_env = AdsrEnvelope::new(sample_rate, patch.filter_adsr[0], patch.filter_adsr[1], patch.filter_adsr[2], patch.filter_adsr[3]);
        
        let pitch_env = ExponEnvelope::new(sample_rate);
        let lfo = Lfo::new(sample_rate, patch.lfo_speed, LfoWaveform::Sine);
        
        Self {
            sample_rate,
            patch,
            osc1_phase: 0.0,
            osc2_phase: 0.0,
            osc2_prev_out: 0.0,
            osc1_env,
            osc2_env,
            filter,
            filter_env,
            pitch_env,
            lfo,
            active: false,
            base_frequency: 440.0,
            note_duration_samples: None,
            samples_played: 0,
            rng_seed: 123456789,
        }
    }

    /// Overwrites the voice's patch configuration and updates all internal DSP modules.
    pub fn set_patch(&mut self, patch: TwoOpPatch) {
        self.patch = patch;
        
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

        self.lfo.set_frequency(patch.lfo_speed);
        self.lfo.waveform = match patch.lfo_waveform {
            0 => LfoWaveform::Sine,
            1 => LfoWaveform::Triangle,
            2 => LfoWaveform::Saw,
            3 => LfoWaveform::Square,
            _ => LfoWaveform::RandomSampleHold,
        };
    }
    
    /// Triggers note playback. Silences automatically after `duration_ms` if greater than zero.
    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }
}

impl Voice for TwoOpVoice {
    /// Triggers note start, resetting envelopes.
    fn note_on(&mut self, frequency: f32, _velocity: f32) {
        self.base_frequency = frequency;
        
        // Reset phases to prevent phase cancellation and keep transients punchy
        self.osc1_phase = 0.0;
        self.osc2_phase = 0.0;
        self.osc2_prev_out = 0.0;
        
        self.osc1_env.trigger_on();
        self.osc2_env.trigger_on();
        self.filter_env.trigger_on();
        
        if self.patch.pitch_sweep_depth.abs() > 0.01 {
            let start_mult = 2.0f32.powf(self.patch.pitch_sweep_depth / 12.0);
            self.pitch_env.trigger(start_mult, 1.0, self.patch.pitch_sweep_decay);
        } else {
            self.pitch_env.trigger(1.0, 1.0, 0.1);
        }
        
        self.active = true;
        self.note_duration_samples = None;
        self.samples_played = 0;
    }

    /// Releases envelopes to start note decay/release fadeout.
    fn note_off(&mut self) {
        self.osc1_env.trigger_off();
        self.osc2_env.trigger_off();
        self.filter_env.trigger_off();
    }

    /// Checks if the voice envelopes are active.
    fn is_active(&self) -> bool {
        self.osc1_env.is_active() || self.osc2_env.is_active()
    }

    /// Sets base oscillator frequency.
    fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
    }
}

impl AudioNode for TwoOpVoice {
    /// Processes a single sample frame, evaluating synthesis algorithm modes and applying filters.
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
        let pitch_mult = self.pitch_env.process();
        let lfo_val = self.lfo.process();
        
        // Pitch envelope modulation
        let current_base_freq = self.base_frequency * pitch_mult;
        
        // Modulate filter cutoff with envelope and LFO
        let mut cutoff = self.patch.filter_cutoff;
        if self.patch.filter_mod_enabled {
            cutoff += self.patch.filter_env_amount * f_env;
        }
        cutoff += self.patch.lfo_cutoff * lfo_val;
        self.filter.set_cutoff(cutoff);
        
        // Modulate FM index with LFO
        let active_mod_index = (self.patch.modulation_index + lfo_val * self.patch.lfo_mod_index).max(0.0);
        
        // Generate phase noise
        self.rng_seed = self.rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise_val = (self.rng_seed as f32) / (u32::MAX as f32) * 2.0 - 1.0;
        
        let carrier_noise_offset = noise_val * self.patch.carrier_noise;
        let modulator_noise_offset = noise_val * self.patch.modulator_noise;

        // Frequencies for Carrier and Modulator
        let osc1_freq = current_base_freq;
        let osc2_freq = current_base_freq * self.patch.osc2_ratio + self.patch.osc2_detune;
        
        // Phase accumulator increments
        let osc1_inc = osc1_freq / self.sample_rate;
        let osc2_inc = osc2_freq / self.sample_rate;
        
        self.osc1_phase = (self.osc1_phase + osc1_inc) % 1.0;
        self.osc2_phase = (self.osc2_phase + osc2_inc) % 1.0;

        // 1. Operator 2 (Modulator) with self-feedback and phase noise
        let op2_phase = (self.osc2_phase + self.patch.osc2_feedback * self.osc2_prev_out + modulator_noise_offset) % 1.0;
        let op2_raw = eval_waveform(self.patch.osc2_waveform, op2_phase);
        self.osc2_prev_out = op2_raw;
        
        let op2_env_sig = op2_raw * env2;
        // Apply wavefolder
        let op2_sig = wavefold(op2_env_sig, self.patch.wavefold_gain, self.patch.wavefold_mix);

        // 2. Carrier and Synth Mode Processing
        let mut sample;
        match self.patch.mode {
            SynthMode::Additive => {
                let op1_phase = (self.osc1_phase + carrier_noise_offset) % 1.0;
                let op1_sig = eval_waveform(self.patch.osc1_waveform, op1_phase) * env1;
                sample = op1_sig * 0.5 + op2_sig * 0.5;
            }
            SynthMode::Am => {
                let op1_phase = (self.osc1_phase + carrier_noise_offset) % 1.0;
                let op1_sig = eval_waveform(self.patch.osc1_waveform, op1_phase);
                sample = op1_sig * (1.0 + op2_sig * active_mod_index) * env1;
            }
            SynthMode::Rm => {
                let op1_phase = (self.osc1_phase + carrier_noise_offset) % 1.0;
                let op1_sig = eval_waveform(self.patch.osc1_waveform, op1_phase);
                sample = op1_sig * op2_sig * active_mod_index * env1;
            }
            SynthMode::Fm => {
                // Implement PM (Phase Modulation)
                // Note: scaling by base_frequency is omitted in PM to maintain stable modulation depth at different octaves.
                let op1_phase = (self.osc1_phase + op2_sig * active_mod_index + carrier_noise_offset) % 1.0;
                sample = eval_waveform(self.patch.osc1_waveform, op1_phase) * env1;
            }
        }
        
        sample = self.filter.process(sample);
        sample
    }
}
