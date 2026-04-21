use std::f32::consts::PI;
use super::AudioProcessor;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

pub struct BiquadFilter {
    sample_rate: f32,
    pub cutoff: f32,
    pub q_factor: f32,
    pub filter_type: FilterType,
    
    // Normalized Transposed Direct Form II coefficients
    a0: f32, a1: f32, a2: f32, b1: f32, b2: f32,
    
    // State memory
    z1: f32, z2: f32,
}

impl BiquadFilter {
    pub fn new(sample_rate: f32, cutoff: f32, q_factor: f32, filter_type: FilterType) -> Self {
        let mut filter = Self {
            sample_rate,
            cutoff,
            q_factor,
            filter_type,
            a0: 1.0, a1: 0.0, a2: 0.0, b1: 0.0, b2: 0.0,
            z1: 0.0, z2: 0.0,
        };
        filter.calculate_coefficients();
        filter
    }

    pub fn new_lowpass(sample_rate: f32, cutoff: f32, q_factor: f32) -> Self {
        Self::new(sample_rate, cutoff, q_factor, FilterType::LowPass)
    }
    
    pub fn set_cutoff(&mut self, cutoff: f32) {
        // Only recalculate if reasonably different to save CPU
        if (self.cutoff - cutoff).abs() > 0.1 {
            self.cutoff = cutoff.clamp(20.0, self.sample_rate / 2.0 - 1.0);
            self.calculate_coefficients();
        }
    }

    pub fn set_q(&mut self, q_factor: f32) {
        if (self.q_factor - q_factor).abs() > 0.01 {
            self.q_factor = q_factor.max(0.1);
            self.calculate_coefficients();
        }
    }

    pub fn set_type(&mut self, filter_type: FilterType) {
        if self.filter_type != filter_type {
            self.filter_type = filter_type;
            self.calculate_coefficients();
        }
    }
    
    fn calculate_coefficients(&mut self) {
        let w0 = 2.0 * PI * self.cutoff / self.sample_rate;
        let alpha = w0.sin() / (2.0 * self.q_factor);
        let cos_w0 = w0.cos();
        
        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            FilterType::LowPass => (
                (1.0 - cos_w0) / 2.0,
                1.0 - cos_w0,
                (1.0 - cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterType::HighPass => (
                (1.0 + cos_w0) / 2.0,
                -(1.0 + cos_w0),
                (1.0 + cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterType::BandPass => (
                alpha,
                0.0,
                -alpha,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            FilterType::Notch => (
                1.0,
                -2.0 * cos_w0,
                1.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
        };
        
        self.a0 = b0 / a0;
        self.a1 = b1 / a0;
        self.a2 = b2 / a0;
        self.b1 = a1 / a0;
        self.b2 = a2 / a0;
    }
}

impl AudioProcessor for BiquadFilter {
    fn process(&mut self, input: f32) -> f32 {
        let out = self.a0 * input + self.z1;
        self.z1 = self.a1 * input - self.b1 * out + self.z2;
        self.z2 = self.a2 * input - self.b2 * out;
        
        // Prevent denormals
        if out.abs() < 1e-15 { 0.0 } else { out }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biquad_lowpass() {
        let mut filter = BiquadFilter::new_lowpass(44100.0, 1000.0, 0.707);
        // Process a DC signal (1.0). Lowpass should allow DC to pass eventually, 
        // but initial samples will be affected by the filter phase/delay.
        let mut out = 0.0;
        for _ in 0..100 {
            out = filter.process(1.0);
        }
        // At 100 samples, a 1000Hz filter on a 44.1kHz rate doing DC step response 
        // will not have settled to exactly 1.0, but it should be non-zero and positive.
        assert!(out > 0.0);
    }
}
