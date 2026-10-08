use crate::dsp::{
    oscillator::{SubOscillator, PolyBlepOscillator},
    envelope::AdsrEnvelope,
    filter::BaseWidthFilter,
    distortion::Distortion,
    AudioNode, AudioProcessor,
};
use crate::synth::Voice;
use super::{SwaveMode, SwavePatch};

/// A polyphonic voice for `SwaveSynth` modeling the Elektron Monomachine SuperWave engine.
pub struct SwaveVoice {
    sample_rate: f32,
    pub patch: SwavePatch,

    // 5 PolyBlepOscillators: [center, inner_left, inner_right, outer_left, outer_right]
    oscillators: [PolyBlepOscillator; 5],

    // Sub-oscillator
    sub_osc: SubOscillator,

    // Envelopes
    pub amp_env: AdsrEnvelope,
    pub filter_env: AdsrEnvelope,

    // Base & Width Filter (stereo pair)
    filter_l: BaseWidthFilter,
    filter_r: BaseWidthFilter,

    // Distortion
    distortion: Distortion,

    pub active: bool,
    pub base_frequency: f32,
    note_duration_samples: Option<usize>,
    samples_played: usize,
    control_counter: usize,
}

impl SwaveVoice {
    pub fn new(sample_rate: f32) -> Self {
        let patch = SwavePatch::default();
        let amp_env = AdsrEnvelope::new(sample_rate, patch.amp_adsr[0], patch.amp_adsr[1], patch.amp_adsr[2], patch.amp_adsr[3]);
        let filter_env = AdsrEnvelope::new(sample_rate, patch.filter_adsr[0], patch.filter_adsr[1], patch.filter_adsr[2], patch.filter_adsr[3]);

        let sub_osc = SubOscillator::new(sample_rate, 440.0, patch.sub_octave, patch.sub_waveform);

        let mut filter_l = BaseWidthFilter::new(sample_rate);
        let mut filter_r = BaseWidthFilter::new(sample_rate);
        filter_l.set_base_width(patch.filter_base, patch.filter_width);
        filter_r.set_base_width(patch.filter_base, patch.filter_width);
        filter_l.set_resonance(patch.filter_hp_q, patch.filter_lp_q);
        filter_r.set_resonance(patch.filter_hp_q, patch.filter_lp_q);

        let distortion = Distortion::new(patch.drive_mode, patch.drive_gain, patch.drive_mix);

        let oscillators = [
            PolyBlepOscillator::new(sample_rate, 440.0, patch.base_waveform),
            PolyBlepOscillator::new(sample_rate, 440.0, patch.base_waveform),
            PolyBlepOscillator::new(sample_rate, 440.0, patch.base_waveform),
            PolyBlepOscillator::new(sample_rate, 440.0, patch.base_waveform),
            PolyBlepOscillator::new(sample_rate, 440.0, patch.base_waveform),
        ];

        let mut voice = Self {
            sample_rate,
            patch,
            oscillators,
            sub_osc,
            amp_env,
            filter_env,
            filter_l,
            filter_r,
            distortion,
            active: false,
            base_frequency: 440.0,
            note_duration_samples: None,
            samples_played: 0,
            control_counter: 0,
        };
        voice.update_frequencies();
        voice
    }

    fn update_frequencies(&mut self) {
        match self.patch.mode {
            SwaveMode::Saw => {
                let d = self.patch.unison_detune;
                let freqs = [
                    self.base_frequency,
                    self.base_frequency * (1.0 - d),
                    self.base_frequency * (1.0 + d),
                    self.base_frequency * (1.0 - 2.0 * d),
                    self.base_frequency * (1.0 + 2.0 * d),
                ];
                for i in 0..5 {
                    self.oscillators[i].set_frequency(freqs[i]);
                    self.oscillators[i].set_waveform(self.patch.base_waveform);
                }
            }
            SwaveMode::Ensemble => {
                let intervals = [
                    0.0,
                    self.patch.ensemble_intervals[0],
                    self.patch.ensemble_intervals[1],
                    self.patch.ensemble_intervals[2],
                ];
                for i in 0..4 {
                    let freq = self.base_frequency * 2.0f32.powf(intervals[i] / 12.0);
                    self.oscillators[i].set_frequency(freq);
                    self.oscillators[i].set_waveform(self.patch.base_waveform);
                }
            }
        }
        self.sub_osc.set_frequency(self.base_frequency);
    }

    pub fn set_patch(&mut self, patch: SwavePatch) {
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

        self.sub_osc.waveform = patch.sub_waveform;
        self.sub_osc.octave_offset = patch.sub_octave;

        self.filter_l.set_base_width(patch.filter_base, patch.filter_width);
        self.filter_r.set_base_width(patch.filter_base, patch.filter_width);
        self.filter_l.set_resonance(patch.filter_hp_q, patch.filter_lp_q);
        self.filter_r.set_resonance(patch.filter_hp_q, patch.filter_lp_q);

        self.distortion.mode = patch.drive_mode;
        self.distortion.drive = patch.drive_gain;
        self.distortion.mix = patch.drive_mix;

        self.update_frequencies();
    }

    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        self.note_on(frequency, velocity);
        if duration_ms > 0.0 {
            self.note_duration_samples = Some((duration_ms * self.sample_rate / 1000.0) as usize);
        }
    }

    /// Process a single stereo frame for this voice, returning `(left, right)`.
    pub fn process_stereo(&mut self) -> (f32, f32) {
        if !self.is_active() {
            self.active = false;
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

        let amp = self.amp_env.process();
        let f_env = self.filter_env.process();

        // Control rate modulation of Base/Width filter every 8 samples
        self.control_counter += 1;
        if self.control_counter % 8 == 0 {
            let base = (self.patch.filter_base + self.patch.filter_env_amount * f_env).clamp(10.0, self.sample_rate * 0.45);
            self.filter_l.set_base_width(base, self.patch.filter_width);
            self.filter_r.set_base_width(base, self.patch.filter_width);
        }

        let mut left_osc = 0.0;
        let mut right_osc = 0.0;

        match self.patch.mode {
            SwaveMode::Saw => {
                // 5-Oscillator SuperWave Saw Cluster
                // Center (0), Inner Left (1), Inner Right (2), Outer Left (3), Outer Right (4)
                let spread = self.patch.stereo_spread;
                let levels = [
                    1.0,
                    self.patch.unison_level,
                    self.patch.unison_level,
                    self.patch.unison_ext_level,
                    self.patch.unison_ext_level,
                ];

                for i in 0..5 {
                    let s = self.oscillators[i].process() * levels[i];

                    let pan = match i {
                        0 => 0.0,
                        1 => -0.5 * spread,
                        2 => 0.5 * spread,
                        3 => -1.0 * spread,
                        4 => 1.0 * spread,
                        _ => 0.0,
                    };

                    let pan_norm = (pan + 1.0) * 0.5; // [0.0, 1.0]
                    left_osc += s * (1.0 - pan_norm);
                    right_osc += s * pan_norm;
                }
            }
            SwaveMode::Ensemble => {
                // 4-Oscillator Chord / Ensemble
                let spread = self.patch.stereo_spread;
                for i in 0..4 {
                    let gain = if i == 0 { 1.0 } else { self.patch.ensemble_level };
                    let s = self.oscillators[i].process() * gain;

                    let pan = match i {
                        0 => 0.0,
                        1 => -0.8 * spread,
                        2 => 0.8 * spread,
                        3 => 0.0,
                        _ => 0.0,
                    };
                    let pan_norm = (pan + 1.0) * 0.5;
                    left_osc += s * (1.0 - pan_norm);
                    right_osc += s * pan_norm;
                }
            }
        }

        // Sub-oscillator (solid center mono punch)
        let sub_sample = self.sub_osc.process() * self.patch.sub_gain;
        left_osc += sub_sample;
        right_osc += sub_sample;

        // Apply amplitude envelope
        left_osc *= amp;
        right_osc *= amp;

        // Base & Width Filtering
        let filtered_l = self.filter_l.process(left_osc);
        let filtered_r = self.filter_r.process(right_osc);

        // Saturation / Drive
        let out_l = self.distortion.process(filtered_l);
        let out_r = self.distortion.process(filtered_r);

        (out_l, out_r)
    }
}

impl Voice for SwaveVoice {
    fn note_on(&mut self, frequency: f32, _velocity: f32) {
        self.base_frequency = frequency;
        self.update_frequencies();

        self.amp_env.trigger_on();
        self.filter_env.trigger_on();

        self.active = true;
        self.note_duration_samples = None;
        self.samples_played = 0;
    }

    fn note_off(&mut self) {
        self.amp_env.trigger_off();
        self.filter_env.trigger_off();
    }

    fn is_active(&self) -> bool {
        self.amp_env.is_active()
    }

    fn set_frequency(&mut self, frequency: f32) {
        self.base_frequency = frequency;
        self.update_frequencies();
    }
}

impl crate::dsp::AudioNode for SwaveVoice {
    fn process(&mut self) -> f32 {
        let (l, r) = self.process_stereo();
        (l + r) * 0.5
    }
}
