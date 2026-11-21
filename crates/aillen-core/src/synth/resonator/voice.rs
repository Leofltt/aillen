use std::f32::consts::{PI, SQRT_2};
use crate::dsp::envelope::AdsrEnvelope;
use crate::dsp::filter::biquad::{BiquadFilter, FilterType};
use crate::dsp::wavefolder::Wavefolder;
use crate::dsp::bitcrusher::Bitcrusher;
use crate::dsp::{AudioNode, AudioProcessor};

const NUM_MODES: usize = 6;
const MODE_WEIGHTS: [f32; NUM_MODES] = [1.0, 0.72, 0.55, 0.42, 0.32, 0.24];

/// Patch parameters for the Circuit-Bent Karplus-Strong & Modal Resonator Synth.
#[derive(Clone, Debug)]
pub struct ResonatorPatch {
    /// ADSR envelope for exciter noise burst duration and amplitude [attack, decay, sustain, release] in seconds.
    pub exciter_adsr: [f32; 4],
    /// Filter cutoff for exciter noise burst in Hz.
    pub exciter_cutoff: f32,
    /// Decay feedback gain for Karplus-Strong string and modal bank decay time (0.0 to 0.999).
    pub feedback: f32,
    /// Lowpass dampening cutoff in Hz inside the string loop and scaling modal high-partial decay (200.0 to 20000.0).
    pub dampening: f32,
    /// Post-mix per-voice circuit-bent wavefolder drive (1.0 to 10.0).
    pub bend_drive: f32,
    /// Post-mix per-voice circuit-bent wavefolder folds (0.0 = clean, >0.0 = metallic fold).
    pub bend_folds: f32,
    /// Post-mix per-voice circuit-bent bitcrusher quantization bits (1.0 to 16.0, 16.0 = bypass).
    pub bend_bits: f32,
    /// Overtone stretch ratio for the parallel modal bank with f0 anchored (0.5 to 8.0).
    pub modal_ratio: f32,
    /// Equal-power crossfade mix between Karplus-Strong string (0.0) and modal bank (1.0).
    pub modal_mix: f32,
    /// Stereo spread for odd/even modal overtones (0.0 = mono, 1.0 = full L/R spread).
    pub modal_width: f32,
}

impl Default for ResonatorPatch {
    fn default() -> Self {
        Self {
            exciter_adsr: [0.001, 0.03, 0.0, 0.01],
            exciter_cutoff: 4000.0,
            feedback: 0.95,
            dampening: 8000.0,
            bend_drive: 1.0,
            bend_folds: 0.0, // Default clean/unfolded
            bend_bits: 16.0, // Default full 16-bit resolution
            modal_ratio: 1.5,
            modal_mix: 0.2,
            modal_width: 0.0, // Default mono
        }
    }
}

/// Simple PRNG pseudo-random noise generator for the exciter burst.
struct NoiseGen {
    state: u32,
}

impl NoiseGen {
    fn new() -> Self {
        Self { state: 0x12345678 }
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        ((self.state >> 9) as f32 / 8388608.0) - 1.0
    }
}

/// A single coupled-form 2-pole modal resonator with RMS-normalized input gain.
#[derive(Clone, Copy, Default)]
struct ModalMode {
    s1: f32,
    s2: f32,
    rcos: f32,
    rsin: f32,
    gain: f32,
    active: bool,
}

impl ModalMode {
    fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    fn configure(&mut self, sample_rate: f32, freq: f32, t60_sec: f32, weight: f32) {
        if freq < 20.0 || freq >= sample_rate * 0.45 {
            self.active = false;
            self.gain = 0.0;
            self.rcos = 0.0;
            self.rsin = 0.0;
            return;
        }
        self.active = true;
        let w = 2.0 * PI * freq / sample_rate;
        let r = (-6.907755 / (t60_sec * sample_rate).max(10.0))
            .exp()
            .clamp(0.0, 0.99995);
        self.rcos = r * w.cos();
        self.rsin = r * w.sin();
        self.gain = weight * (1.0 - r * r).max(1e-9).sqrt();
    }

    fn process(&mut self, input: f32) -> f32 {
        if !self.active {
            return 0.0;
        }
        let n1 = self.rcos * self.s1 + self.rsin * self.s2;
        let n2 = -self.rsin * self.s1 + self.rcos * self.s2 + self.gain * input;
        self.s1 = if n1.abs() < 1e-15 { 0.0 } else { n1 };
        self.s2 = if n2.abs() < 1e-15 { 0.0 } else { n2 };
        self.s1
    }
}

/// A single voice of the Circuit-Bent Karplus-Strong & Modal Resonator.
pub struct ResonatorVoice {
    sample_rate: f32,
    pub patch: ResonatorPatch,

    exciter_env: AdsrEnvelope,
    exciter_filter: BiquadFilter,
    noise_gen: NoiseGen,

    // Main Karplus-Strong fractional delay line
    delay_buffer: Vec<f32>,
    write_pos: usize,
    loop_dampening: BiquadFilter,
    dc_x1: f32,
    dc_y1: f32,
    dc_r: f32,

    // Parallel 6-mode stereo modal resonator bank
    modal_modes: [ModalMode; NUM_MODES],
    modal_pans: [(f32, f32); NUM_MODES],

    // Post-mix per-voice stereo circuit-bent elements
    bend_wavefolder_l: Wavefolder,
    bend_wavefolder_r: Wavefolder,
    bend_bitcrusher_l: Bitcrusher,
    bend_bitcrusher_r: Bitcrusher,

    pub active: bool,
    pub current_frequency: f32,
    velocity: f32,

    note_duration_samples: Option<usize>,
    samples_played: usize,
    silent_samples: usize,
}

impl ResonatorVoice {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let patch = ResonatorPatch::default();

        let exciter_env = AdsrEnvelope::new(
            sr,
            patch.exciter_adsr[0],
            patch.exciter_adsr[1],
            patch.exciter_adsr[2],
            patch.exciter_adsr[3],
        );
        let exciter_filter = BiquadFilter::new(sr, patch.exciter_cutoff, 0.707, FilterType::LowPass);

        // Buffer length up to 20Hz minimum pitch
        let max_samples = (sr / 20.0).ceil() as usize + 4;
        let delay_buffer = vec![0.0; max_samples];

        let loop_dampening = BiquadFilter::new(sr, patch.dampening, 0.707, FilterType::LowPass);
        let dc_r = 1.0 - (2.0 * PI * 15.0 / sr).min(0.5);

        let bend_wavefolder_l = Wavefolder::new(patch.bend_drive, patch.bend_folds, 0.0);
        let bend_wavefolder_r = Wavefolder::new(patch.bend_drive, patch.bend_folds, 0.0);
        let bend_bitcrusher_l = Bitcrusher::new(patch.bend_bits, 1);
        let bend_bitcrusher_r = Bitcrusher::new(patch.bend_bits, 1);

        let mut voice = Self {
            sample_rate: sr,
            patch,
            exciter_env,
            exciter_filter,
            noise_gen: NoiseGen::new(),
            delay_buffer,
            write_pos: 0,
            loop_dampening,
            dc_x1: 0.0,
            dc_y1: 0.0,
            dc_r,
            modal_modes: [ModalMode::default(); NUM_MODES],
            modal_pans: [(1.0, 1.0); NUM_MODES],
            bend_wavefolder_l,
            bend_wavefolder_r,
            bend_bitcrusher_l,
            bend_bitcrusher_r,
            active: false,
            current_frequency: 220.0,
            velocity: 1.0,
            note_duration_samples: None,
            samples_played: 0,
            silent_samples: 0,
        };
        voice.update_modal_bank();
        voice
    }

    fn update_modal_bank(&mut self) {
        let feedback = self.patch.feedback.clamp(0.0, 0.995);
        let damp = self.patch.dampening.clamp(100.0, (self.sample_rate * 0.45).max(100.0));
        let ratio = self.patch.modal_ratio.clamp(0.1, 16.0);
        let width = self.patch.modal_width.clamp(0.0, 1.0);

        let ref_rate = 180.0 * (self.current_frequency / 220.0).max(0.05).powf(0.3);
        let base_t60 = (-6.907755 / (ref_rate * feedback.max(0.01).ln())).clamp(0.04, 8.0);

        let exp_p = ratio.log2();
        let near_unison = (1.0 - (ratio - 1.0).abs() / 0.12).max(0.0);
        let unison_comp = 1.0 / (1.0 + near_unison);
        let w_norm = MODE_WEIGHTS.iter().map(|w| w * w).sum::<f32>().sqrt();

        for k in 0..NUM_MODES {
            let mult = ((k + 1) as f32).powf(exp_p);
            let detune = 1.0 + (k as f32) * 0.004 * near_unison;
            let f_k = self.current_frequency * mult * detune;
            let damp_factor = 1.0 / (1.0 + (f_k / damp).powf(1.5));
            let t60_k = (base_t60 * damp_factor).max(0.025);
            let sub_comp = if f_k < self.current_frequency {
                (f_k / self.current_frequency.max(20.0)).sqrt().min(1.0)
            } else {
                1.0
            };
            let w_k = (MODE_WEIGHTS[k] / w_norm) * 1.9 * unison_comp * sub_comp;
            self.modal_modes[k].configure(self.sample_rate, f_k, t60_k, w_k);

            let pan = if k == 0 {
                0.0
            } else if k % 2 == 1 {
                -width
            } else {
                width
            };
            let angle = (pan + 1.0) * (PI * 0.25);
            self.modal_pans[k] = (angle.cos() * SQRT_2, angle.sin() * SQRT_2);
        }
    }

    pub fn set_patch(&mut self, patch: ResonatorPatch) {
        self.patch = patch;

        self.exciter_env.attack = self.patch.exciter_adsr[0];
        self.exciter_env.decay = self.patch.exciter_adsr[1];
        self.exciter_env.sustain = self.patch.exciter_adsr[2];
        self.exciter_env.release = self.patch.exciter_adsr[3];
        self.exciter_env.recalculate_rates();

        self.exciter_filter.set_cutoff(self.patch.exciter_cutoff);
        self.loop_dampening.set_cutoff(self.patch.dampening);

        self.bend_wavefolder_l.drive = self.patch.bend_drive;
        self.bend_wavefolder_l.folds = self.patch.bend_folds;
        self.bend_wavefolder_r.drive = self.patch.bend_drive;
        self.bend_wavefolder_r.folds = self.patch.bend_folds;

        self.bend_bitcrusher_l.bits = self.patch.bend_bits;
        self.bend_bitcrusher_r.bits = self.patch.bend_bits;

        self.update_modal_bank();
    }

    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }

    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        self.current_frequency = frequency.clamp(20.0, (self.sample_rate * 0.45).max(20.0));
        self.velocity = velocity.clamp(0.0, 2.0);
        self.exciter_env.value = 0.0;
        self.exciter_env.trigger_on();
        self.active = true;
        self.note_duration_samples = None;
        self.samples_played = 0;
        self.silent_samples = 0;

        self.delay_buffer.fill(0.0);
        self.write_pos = 0;
        self.dc_x1 = 0.0;
        self.dc_y1 = 0.0;
        self.exciter_filter.reset();
        self.loop_dampening.reset();
        self.bend_bitcrusher_l.reset();
        self.bend_bitcrusher_r.reset();
        for mode in &mut self.modal_modes {
            mode.reset();
        }

        self.update_modal_bank();
    }

    pub fn note_off(&mut self) {
        self.exciter_env.trigger_off();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.current_frequency = frequency.clamp(20.0, (self.sample_rate * 0.45).max(20.0));
        self.update_modal_bank();
    }

    /// Processes a single stereo frame:
    /// Exciter -> Parallel (Karplus-Strong String + 6-Mode Stereo Modal Bank) -> Equal-Power Mix -> Stereo Wavefolder + Bitcrusher.
    pub fn process_stereo(&mut self) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }

        if let Some(target_samples) = self.note_duration_samples {
            if self.samples_played >= target_samples {
                self.note_off();
                self.note_duration_samples = None;
            } else {
                self.samples_played += 1;
            }
        }

        // 1. Generate Exciter Noise Burst (scaled by velocity)
        let exciter_amp = self.exciter_env.process();
        let raw_noise = self.noise_gen.next_f32();
        let exciter_sample = self.exciter_filter.process(raw_noise) * exciter_amp * self.velocity;

        // 2. Branch A: Karplus-Strong Fractional Delay Line + Dampening LPF + DC Blocker
        let delay_samples = (self.sample_rate / self.current_frequency)
            .clamp(2.0, (self.delay_buffer.len() - 2) as f32);
        let delay_int = delay_samples.floor() as usize;
        let delay_frac = delay_samples - (delay_int as f32);

        let buf_len = self.delay_buffer.len();
        let read_pos1 = (self.write_pos + buf_len - delay_int) % buf_len;
        let read_pos2 = (self.write_pos + buf_len - delay_int - 1) % buf_len;

        let delayed_sample =
            self.delay_buffer[read_pos1] * (1.0 - delay_frac) + self.delay_buffer[read_pos2] * delay_frac;
        let damped = self.loop_dampening.process(delayed_sample);
        let dc_out = damped - self.dc_x1 + self.dc_r * self.dc_y1;
        self.dc_x1 = damped;
        self.dc_y1 = if dc_out.abs() < 1e-15 { 0.0 } else { dc_out };

        let string_sample =
            (exciter_sample + self.dc_y1 * self.patch.feedback.clamp(0.0, 0.995)).tanh();
        self.delay_buffer[self.write_pos] = string_sample;
        self.write_pos = (self.write_pos + 1) % buf_len;

        // 3. Branch B: True Parallel 6-Mode Stereo Modal Bank
        let mut modal_sum_l = 0.0;
        let mut modal_sum_r = 0.0;
        for k in 0..NUM_MODES {
            let m = self.modal_modes[k].process(exciter_sample);
            let (gl, gr) = self.modal_pans[k];
            modal_sum_l += m * gl;
            modal_sum_r += m * gr;
        }
        let modal_l = modal_sum_l.tanh();
        let modal_r = modal_sum_r.tanh();

        // 4. Equal-Power Crossfade Mix
        let mix_angle = self.patch.modal_mix.clamp(0.0, 1.0) * (PI * 0.5);
        let string_gain = mix_angle.cos();
        let modal_gain = mix_angle.sin();
        let mixed_l = string_sample * string_gain + modal_l * modal_gain;
        let mixed_r = string_sample * string_gain + modal_r * modal_gain;

        // 5. Post-Mix Per-Voice Stereo Circuit Bending
        let folded_l = self.bend_wavefolder_l.process(mixed_l);
        let folded_r = self.bend_wavefolder_r.process(mixed_r);
        let out_l = self.bend_bitcrusher_l.process(folded_l);
        let out_r = self.bend_bitcrusher_r.process(folded_r);

        // 6. Consecutive-sample silence detector (50 ms) to avoid zero-crossing kills
        let silence_limit = ((self.sample_rate * 0.05) as usize).max(1);
        if exciter_amp <= 0.0001
            && out_l.abs() < 1e-4
            && out_r.abs() < 1e-4
            && string_sample.abs() < 1e-4
            && modal_l.abs() < 1e-4
            && modal_r.abs() < 1e-4
        {
            self.silent_samples += 1;
            if self.silent_samples >= silence_limit {
                self.active = false;
                self.delay_buffer.fill(0.0);
                for mode in &mut self.modal_modes {
                    mode.reset();
                }
            }
        } else {
            self.silent_samples = 0;
        }

        (
            if out_l.abs() < 1e-15 { 0.0 } else { out_l },
            if out_r.abs() < 1e-15 { 0.0 } else { out_r },
        )
    }
}

impl AudioNode for ResonatorVoice {
    fn process(&mut self) -> f32 {
        let (l, r) = self.process_stereo();
        (l + r) * 0.5
    }
}
