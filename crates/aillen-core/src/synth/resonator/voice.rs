use crate::dsp::envelope::AdsrEnvelope;
use crate::dsp::filter::biquad::{BiquadFilter, FilterType};
use crate::dsp::wavefolder::Wavefolder;
use crate::dsp::bitcrusher::Bitcrusher;
use crate::dsp::{AudioNode, AudioProcessor};

/// Patch parameters for the Circuit-Bent Karplus-Strong Resonator Synth.
#[derive(Clone, Debug)]
pub struct ResonatorPatch {
    /// ADSR envelope for exciter noise burst duration and amplitude [attack, decay, sustain, release] in seconds.
    pub exciter_adsr: [f32; 4],
    /// Filter cutoff for exciter noise burst in Hz.
    pub exciter_cutoff: f32,
    /// Feedback gain for Karplus-Strong delay line (0.0 to 0.999). High values produce long metallic decay.
    pub feedback: f32,
    /// Lowpass dampening filter cutoff inside feedback loop in Hz (200.0 to 20000.0).
    pub dampening: f32,
    /// Circuit-bent wavefolder drive inside feedback loop (1.0 to 10.0).
    pub bend_drive: f32,
    /// Circuit-bent wavefolder folds inside feedback loop (0.0 = clean, >0.0 = metallic fold).
    pub bend_folds: f32,
    /// Circuit-bent bitcrusher quantization bits inside feedback loop (1.0 to 16.0, 16.0 = bypass).
    pub bend_bits: f32,
    /// Inharmonic detune ratio applied to parallel modal resonator (0.5 to 4.0).
    pub modal_ratio: f32,
    /// Mix level of the modal resonator (0.0 to 1.0).
    pub modal_mix: f32,
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

/// A single voice of the Circuit-Bent Karplus-Strong Resonator.
pub struct ResonatorVoice {
    sample_rate: f32,
    pub patch: ResonatorPatch,

    exciter_env: AdsrEnvelope,
    exciter_filter: BiquadFilter,
    noise_gen: NoiseGen,

    // Main Karplus-Strong fractional delay line
    delay_buffer: Vec<f32>,
    write_pos: usize,

    // Circuit-bent elements in feedback loop
    loop_dampening: BiquadFilter,
    loop_wavefolder: Wavefolder,
    loop_bitcrusher: Bitcrusher,

    // Secondary parallel modal resonator filter
    modal_filter: BiquadFilter,

    pub active: bool,
    pub current_frequency: f32,

    note_duration_samples: Option<usize>,
    samples_played: usize,
}

impl ResonatorVoice {
    pub fn new(sample_rate: f32) -> Self {
        let patch = ResonatorPatch::default();

        let exciter_env = AdsrEnvelope::new(
            sample_rate,
            patch.exciter_adsr[0],
            patch.exciter_adsr[1],
            patch.exciter_adsr[2],
            patch.exciter_adsr[3],
        );
        let exciter_filter = BiquadFilter::new(sample_rate, patch.exciter_cutoff, 0.707, FilterType::LowPass);

        // Buffer length up to 20Hz minimum pitch
        let max_samples = (sample_rate / 20.0).ceil() as usize + 4;
        let delay_buffer = vec![0.0; max_samples];

        let loop_dampening = BiquadFilter::new(sample_rate, patch.dampening, 0.707, FilterType::LowPass);
        let loop_wavefolder = Wavefolder::new(patch.bend_drive, patch.bend_folds, 0.0);
        let loop_bitcrusher = Bitcrusher::new(patch.bend_bits, 1);

        let modal_filter = BiquadFilter::new(sample_rate, 440.0 * patch.modal_ratio, 2.5, FilterType::BandPass);

        Self {
            sample_rate,
            patch,
            exciter_env,
            exciter_filter,
            noise_gen: NoiseGen::new(),
            delay_buffer,
            write_pos: 0,
            loop_dampening,
            loop_wavefolder,
            loop_bitcrusher,
            modal_filter,
            active: false,
            current_frequency: 220.0,
            note_duration_samples: None,
            samples_played: 0,
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

        self.loop_wavefolder.drive = self.patch.bend_drive;
        self.loop_wavefolder.folds = self.patch.bend_folds;

        self.loop_bitcrusher.bits = self.patch.bend_bits;

        self.modal_filter.set_cutoff((self.current_frequency * self.patch.modal_ratio).clamp(20.0, 20000.0));
    }


    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }

    pub fn note_on(&mut self, frequency: f32, _velocity: f32) {
        self.current_frequency = frequency.max(20.0);
        self.exciter_env.trigger_on();
        self.active = true;
        self.note_duration_samples = None;
        self.samples_played = 0;

        // Reset delay line and seed with immediate excitation noise burst
        self.delay_buffer.fill(0.0);
        self.write_pos = 0;
        let delay_len = (self.sample_rate / self.current_frequency).clamp(2.0, (self.delay_buffer.len() - 2) as f32) as usize;
        for i in 0..delay_len {
            let noise = self.noise_gen.next_f32();
            self.delay_buffer[i] = self.exciter_filter.process(noise) * 0.8;
        }

        self.modal_filter.set_cutoff((self.current_frequency * self.patch.modal_ratio).clamp(20.0, 20000.0));
    }

    pub fn note_off(&mut self) {
        self.exciter_env.trigger_off();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.current_frequency = frequency.max(20.0);
        self.modal_filter.set_cutoff((self.current_frequency * self.patch.modal_ratio).clamp(20.0, 20000.0));
    }
}

impl AudioNode for ResonatorVoice {
    fn process(&mut self) -> f32 {
        if !self.active {
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

        // 1. Generate Exciter Noise Burst
        let exciter_amp = self.exciter_env.process();
        let raw_noise = self.noise_gen.next_f32();
        let exciter_sample = self.exciter_filter.process(raw_noise) * exciter_amp;

        // 2. Read from tuned Karplus-Strong Fractional Delay Line
        let delay_samples = (self.sample_rate / self.current_frequency).clamp(2.0, (self.delay_buffer.len() - 2) as f32);
        let delay_int = delay_samples.floor() as usize;
        let delay_frac = delay_samples - (delay_int as f32);

        let buf_len = self.delay_buffer.len();
        let read_pos1 = (self.write_pos + buf_len - delay_int) % buf_len;
        let read_pos2 = (self.write_pos + buf_len - delay_int - 1) % buf_len;

        let delayed_sample = self.delay_buffer[read_pos1] * (1.0 - delay_frac) + self.delay_buffer[read_pos2] * delay_frac;

        // 3. Process Circuit-Bent Feedback Loop: Wavefolder -> Bitcrusher -> Dampening Lowpass
        let folded = self.loop_wavefolder.process(delayed_sample);
        let crushed = self.loop_bitcrusher.process(folded);
        let damped = self.loop_dampening.process(crushed);

        // Feedback calculation with soft clipping to prevent runaway oscillation
        let feedback_sample = (exciter_sample + damped * self.patch.feedback.clamp(0.0, 0.995)).tanh();
        self.delay_buffer[self.write_pos] = feedback_sample;

        self.write_pos = (self.write_pos + 1) % buf_len;

        // 4. Modal Resonator parallel tap
        let modal_out = self.modal_filter.process(feedback_sample);

        // Combine string output with modal resonator
        let output = feedback_sample * (1.0 - self.patch.modal_mix) + modal_out * self.patch.modal_mix;

        // Silence check if excitation died out and output is near zero
        if exciter_amp <= 0.0001 && output.abs() < 1e-4 {
            self.active = false;
            self.delay_buffer.fill(0.0);
        }

        output
    }
}
