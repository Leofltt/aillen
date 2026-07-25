use crate::dsp::{
    envelope::AdsrEnvelope,
    filter::ResonantLadderFilter,
    AudioNode, AudioProcessor,
    oscillator::Waveform,
};
use crate::synth::Voice;
use super::Synth303Patch;
use std::f32::consts::PI;

/// Band-limited oscillator supporting Sawtooth, Square, and PWM/PW modulation.
pub struct Synth303Oscillator {
    pub sample_rate: f32,
    pub phase: f32,
    pub pwm_phase: f32,
}

impl Synth303Oscillator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            phase: 0.0,
            pwm_phase: 0.0,
        }
    }

    fn poly_blep(t: f32, dt: f32) -> f32 {
        if t < dt {
            let t = t / dt;
            t + t - t * t - 1.0
        } else if t > 1.0 - dt {
            let t = (t - 1.0) / dt;
            t * t + t + t + 1.0
        } else {
            0.0
        }
    }

    pub fn process(&mut self, frequency: f32, waveform: Waveform, pw: f32, pwm_rate: f32, pwm_depth: f32) -> f32 {
        let dt = frequency / self.sample_rate;
        
        // Update PWM LFO
        let pwm_dt = pwm_rate / self.sample_rate;
        self.pwm_phase += pwm_dt;
        if self.pwm_phase >= 1.0 {
            self.pwm_phase -= 1.0;
        }
        
        // Modulate pulse width
        let lfo = (self.pwm_phase * 2.0 * PI).sin();
        let effective_pw = (pw + lfo * pwm_depth * 0.4).clamp(0.05, 0.95);

        let sample = match waveform {
            Waveform::Square => {
                let naive = if self.phase < effective_pw { 1.0 } else { -1.0 };
                
                // PolyBLEP correction at 0.0 (rising transition)
                let corr1 = Self::poly_blep(self.phase, dt);
                
                // PolyBLEP correction at effective_pw (falling transition)
                let mut phase2 = self.phase - effective_pw;
                if phase2 < 0.0 {
                    phase2 += 1.0;
                }
                let corr2 = Self::poly_blep(phase2, dt);
                
                naive + corr1 - corr2
            }
            Waveform::Saw => {
                // If pwm_depth > 0, we mix a second phase-shifted saw to get PWM saw texture
                let naive1 = 2.0 * self.phase - 1.0;
                let corr1 = Self::poly_blep(self.phase, dt);
                let saw1 = naive1 - corr1;

                if pwm_depth > 0.01 {
                    let phase2 = (self.phase + effective_pw) % 1.0;
                    let naive2 = 2.0 * phase2 - 1.0;
                    let corr2 = Self::poly_blep(phase2, dt);
                    let saw2 = naive2 - corr2;
                    // Blend based on pwm_depth
                    saw1 * (1.0 - pwm_depth * 0.5) + saw2 * (pwm_depth * 0.5)
                } else {
                    saw1
                }
            }
            Waveform::Sine => {
                (self.phase * 2.0 * PI).sin()
            }
            Waveform::Triangle => {
                1.0 - 4.0 * (self.phase - 0.5).abs()
            }
        };

        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }
}

/// A single monophonic/legato-capable voice for `Synth303`.
pub struct Synth303Voice {
    sample_rate: f32,
    pub patch: Synth303Patch,

    pub osc: Synth303Oscillator,
    pub amp_env: AdsrEnvelope,
    pub filter_env: AdsrEnvelope,
    pub pitch_env: AdsrEnvelope,

    pub filter: ResonantLadderFilter,

    pub active: bool,
    pub base_frequency: f32,
    pub current_frequency: f32,

    note_duration_samples: Option<usize>,
    samples_played: usize,
}

impl Synth303Voice {
    pub fn new(sample_rate: f32) -> Self {
        let patch = Synth303Patch::default();
        let osc = Synth303Oscillator::new(sample_rate);
        let amp_env = AdsrEnvelope::new(sample_rate, patch.amp_adsr[0], patch.amp_adsr[1], patch.amp_adsr[2], patch.amp_adsr[3]);
        let filter_env = AdsrEnvelope::new(sample_rate, patch.filter_adsr[0], patch.filter_adsr[1], patch.filter_adsr[2], patch.filter_adsr[3]);
        let pitch_env = AdsrEnvelope::new(sample_rate, patch.pitch_adsr[0], patch.pitch_adsr[1], patch.pitch_adsr[2], patch.pitch_adsr[3]);
        let filter = ResonantLadderFilter::new(sample_rate);

        Self {
            sample_rate,
            patch,
            osc,
            amp_env,
            filter_env,
            pitch_env,
            filter,
            active: false,
            base_frequency: 220.0,
            current_frequency: 220.0,
            note_duration_samples: None,
            samples_played: 0,
        }
    }

    pub fn set_patch(&mut self, patch: Synth303Patch) {
        self.patch = patch;

        self.amp_env.attack = patch.amp_adsr[0];
        self.amp_env.decay = patch.amp_adsr[1];
        self.amp_env.sustain = patch.amp_adsr[2];
        self.amp_env.release = patch.amp_adsr[3];
        self.amp_env.recalculate_rates();

        self.filter_env.attack = patch.filter_adsr[0];
        self.filter_env.decay = patch.filter_adsr[1];
        self.filter_env.sustain = patch.filter_adsr[2];
        self.filter_env.release = patch.filter_adsr[3];
        self.filter_env.recalculate_rates();

        self.pitch_env.attack = patch.pitch_adsr[0];
        self.pitch_env.decay = patch.pitch_adsr[1];
        self.pitch_env.sustain = patch.pitch_adsr[2];
        self.pitch_env.release = patch.pitch_adsr[3];
        self.pitch_env.recalculate_rates();
    }

    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }
}

impl Voice for Synth303Voice {
    fn note_on(&mut self, frequency: f32, _velocity: f32) {
        // If already active, we glide/portamento to the new frequency instead of retriggering envelopes
        if self.active {
            self.base_frequency = frequency;
            // Envelopes do not retrigger if legato
        } else {
            self.base_frequency = frequency;
            self.current_frequency = frequency;
            self.amp_env.trigger_on();
            self.filter_env.trigger_on();
            self.pitch_env.trigger_on();
            self.active = true;
        }
        self.note_duration_samples = None;
        self.samples_played = 0;
    }

    fn note_off(&mut self) {
        self.amp_env.trigger_off();
        self.filter_env.trigger_off();
        self.pitch_env.trigger_off();
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
    }
}

impl AudioNode for Synth303Voice {
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

        // Apply frequency glide (Portamento)
        if self.patch.glide_time > 0.001 {
            let glide_coeff = 1.0 - (-1.0 / (self.patch.glide_time * self.sample_rate)).exp();
            self.current_frequency += (self.base_frequency - self.current_frequency) * glide_coeff;
        } else {
            self.current_frequency = self.base_frequency;
        }

        // Process envelopes
        let amp_val = self.amp_env.process();
        let filter_val = self.filter_env.process();
        let pitch_val = self.pitch_env.process();

        // Calculate pitch modulation
        let pitch_mod = self.patch.pitch_env_amount * pitch_val;
        let final_freq = (self.current_frequency + pitch_mod).max(20.0);

        // Process oscillator
        let osc_out = self.osc.process(
            final_freq,
            self.patch.waveform,
            self.patch.pulse_width,
            self.patch.pwm_rate,
            self.patch.pwm_depth,
        );

        // Apply filter with envelope modulation
        let cutoff = (self.patch.filter_cutoff + self.patch.filter_env_amount * filter_val).max(20.0);
        self.filter.set_params(cutoff, self.patch.filter_resonance);
        let filtered_out = self.filter.process(osc_out);

        // Apply amp envelope
        let signal = filtered_out * amp_val;
        
        // Soft-clip saturation: drive the signal slightly, shape with tanh, and scale back
        (signal * 3.5).tanh() * 0.85
    }
}
