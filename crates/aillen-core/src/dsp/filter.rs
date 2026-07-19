use std::f32::consts::PI;
use super::AudioProcessor;

/// Supported types of biquad filter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterType {
    /// Low pass filter (attenuates high frequencies).
    LowPass,
    /// High pass filter (attenuates low frequencies).
    HighPass,
    /// Band pass filter (passes only frequencies in a narrow band).
    BandPass,
    /// Notch / Band reject filter (attenuates only frequencies in a narrow band).
    Notch,
}

/// A standard 2-pole Biquad IIR Filter.
/// Implemented using Normalized Transposed Direct Form II to prevent internal clip errors.
pub struct BiquadFilter {
    sample_rate: f32,
    /// Active cutoff frequency in Hz.
    pub cutoff: f32,
    /// Quality factor determining resonance level (typically 0.707 for flat response).
    pub q_factor: f32,
    /// Filter type.
    pub filter_type: FilterType,
    
    // Normalized Transposed Direct Form II coefficients
    a0: f32, 
    a1: f32, 
    a2: f32, 
    b1: f32, 
    b2: f32,
    
    // State memory registers
    z1: f32, 
    z2: f32,
}

impl BiquadFilter {
    /// Creates a new `BiquadFilter` instance.
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

    /// Convenience initializer to create a LowPass filter.
    pub fn new_lowpass(sample_rate: f32, cutoff: f32, q_factor: f32) -> Self {
        Self::new(sample_rate, cutoff, q_factor, FilterType::LowPass)
    }
    
    /// Sets the filter cutoff frequency. recalculates coefficients dynamically.
    pub fn set_cutoff(&mut self, cutoff: f32) {
        if (self.cutoff - cutoff).abs() > 0.1 {
            self.cutoff = cutoff.clamp(20.0, self.sample_rate / 2.0 - 1.0);
            self.calculate_coefficients();
        }
    }

    /// Sets the Q (resonance) factor.
    pub fn set_q(&mut self, q_factor: f32) {
        if (self.q_factor - q_factor).abs() > 0.01 {
            self.q_factor = q_factor.max(0.1);
            self.calculate_coefficients();
        }
    }

    /// Sets the filter type.
    pub fn set_type(&mut self, filter_type: FilterType) {
        if self.filter_type != filter_type {
            self.filter_type = filter_type;
            self.calculate_coefficients();
        }
    }
    
    /// Recalculates filter coefficients based on current sample rate, cutoff, Q, and filter type.
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
    /// Filters a single mono input sample frame and updates state.
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
        let mut out = 0.0;
        for _ in 0..100 {
            out = filter.process(1.0);
        }
        assert!(out > 0.0);
    }
}
