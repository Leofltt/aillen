use crate::dsp::filter::biquad::{BiquadFilter, FilterType};
use crate::dsp::AudioProcessor;

/// A parallel multi-peak Formant filter for vocal vowel morphing effects (A, E, I, O, U).
pub struct FormantFilter {
    f1_filter: BiquadFilter,
    f2_filter: BiquadFilter,
    f3_filter: BiquadFilter,
    /// Morph parameter from 0.0 to 1.0 spanning vowels [A, E, I, O, U]
    pub vowel: f32,
}

impl FormantFilter {
    pub fn new(sample_rate: f32) -> Self {
        let f1 = BiquadFilter::new(sample_rate, 730.0, 12.0, FilterType::BandPass);
        let f2 = BiquadFilter::new(sample_rate, 1090.0, 10.0, FilterType::BandPass);
        let f3 = BiquadFilter::new(sample_rate, 2440.0, 8.0, FilterType::BandPass);

        Self {
            f1_filter: f1,
            f2_filter: f2,
            f3_filter: f3,
            vowel: 0.0,
        }
    }

    /// Set the vowel parameter (0.0 to 1.0) and update formant peak frequencies.
    pub fn set_vowel(&mut self, vowel: f32) {
        self.vowel = vowel.clamp(0.0, 1.0);
        
        // Define center frequencies for F1, F2, F3 across A, E, I, O, U
        let f1_presets = [730.0, 530.0, 270.0, 300.0, 440.0];
        let f2_presets = [1090.0, 1840.0, 2290.0, 870.0, 1020.0];
        let f3_presets = [2440.0, 2480.0, 3010.0, 2240.0, 2240.0];

        // Interpolate across 5 vowel points
        let scaled = self.vowel * 4.0;
        let index = scaled.floor() as usize;
        let frac = scaled - scaled.floor();

        let idx0 = index.min(4);
        let idx1 = (index + 1).min(4);

        let f1 = f1_presets[idx0] * (1.0 - frac) + f1_presets[idx1] * frac;
        let f2 = f2_presets[idx0] * (1.0 - frac) + f2_presets[idx1] * frac;
        let f3 = f3_presets[idx0] * (1.0 - frac) + f3_presets[idx1] * frac;

        self.f1_filter.set_cutoff(f1);
        self.f2_filter.set_cutoff(f2);
        self.f3_filter.set_cutoff(f3);
    }
}

impl AudioProcessor for FormantFilter {
    fn process(&mut self, input: f32) -> f32 {
        // Run input through the parallel filters
        let out1 = self.f1_filter.process(input);
        let out2 = self.f2_filter.process(input);
        let out3 = self.f3_filter.process(input);

        // Mix the bandpass peaks. Higher formants are mixed lower to sound balanced.
        let output = out1 + out2 * 0.7 + out3 * 0.4;
        
        // Prevent denormals
        if output.abs() < 1e-15 { 0.0 } else { output }
    }
}
