use crate::dsp::AudioProcessor;

/// A sidechainable compressor that performs dynamic range compression.
/// It can be used as a standard inline compressor (via AudioProcessor::process)
/// or as a sidechain compressor (via process_sidechain).
pub struct Compressor {
    sample_rate: f32,
    /// Threshold in decibels (dB), below which compression is not applied.
    pub threshold: f32,
    /// Compression ratio (e.g. 4.0 for 4:1 compression).
    pub ratio: f32,
    /// Attack time in seconds (how fast the compressor responds to signals above threshold).
    pub attack: f32,
    /// Release time in seconds (how fast the compressor recovers after the signal drops below threshold).
    pub release: f32,
    /// Makeup gain in decibels (dB) applied to the output.
    pub makeup_gain: f32,
    
    // Internal envelope follower state
    envelope: f32,
}

impl Compressor {
    /// Creates a new `Compressor` with default settings:
    /// - threshold: -24.0 dB
    /// - ratio: 4.0
    /// - attack: 0.01 seconds (10ms)
    /// - release: 0.1 seconds (100ms)
    /// - makeup_gain: 0.0 dB
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            threshold: -24.0,
            ratio: 4.0,
            attack: 0.01,
            release: 0.1,
            makeup_gain: 0.0,
            envelope: 0.0,
        }
    }

    /// Processes a single frame of audio with an external sidechain signal.
    pub fn process_sidechain(&mut self, input: f32, sidechain: f32) -> f32 {
        // Calculate attack/release time constants
        let att_coef = if self.attack > 0.0 {
            (-1.0 / (self.attack * self.sample_rate)).exp()
        } else {
            0.0
        };
        let rel_coef = if self.release > 0.0 {
            (-1.0 / (self.release * self.sample_rate)).exp()
        } else {
            0.0
        };

        // Envelope follower on the sidechain signal (detecting peak)
        let rect = sidechain.abs();
        if rect > self.envelope {
            self.envelope = att_coef * self.envelope + (1.0 - att_coef) * rect;
        } else {
            self.envelope = rel_coef * self.envelope + (1.0 - rel_coef) * rect;
        }

        // Convert envelope amplitude to dB
        let env_db = if self.envelope > 1e-5 {
            20.0 * self.envelope.log10()
        } else {
            -100.0
        };

        // Calculate gain reduction in dB
        let gain_reduction_db = if env_db > self.threshold {
            (self.threshold - env_db) * (1.0 - 1.0 / self.ratio.max(1.0))
        } else {
            0.0
        };

        // Total gain in dB (reduction + makeup)
        let total_gain_db = gain_reduction_db + self.makeup_gain;

        // Convert back to linear factor
        let gain = 10.0_f32.powf(total_gain_db / 20.0);

        let out = input * gain;
        
        // Prevent denormals
        if out.abs() < 1e-15 { 0.0 } else { out }
    }
}

impl AudioProcessor for Compressor {
    /// Filters a single mono input sample frame (compressing it based on its own signal level) and updates state.
    fn process(&mut self, input: f32) -> f32 {
        self.process_sidechain(input, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_no_compression() {
        let mut comp = Compressor::new(44100.0);
        comp.threshold = 0.0; // Very high threshold
        comp.ratio = 1.0;     // No ratio
        
        // Input should pass through unchanged
        let out = comp.process(0.5);
        assert!((out - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_compressor_reduction() {
        let mut comp = Compressor::new(44100.0);
        comp.threshold = -20.0;
        comp.ratio = 4.0;
        comp.attack = 0.001; // Fast attack
        
        // Send a very hot signal for many samples to trigger envelope follower
        let mut out = 0.0;
        for _ in 0..100 {
            out = comp.process(1.0); // 0 dB signal is above -20 dB threshold
        }
        
        // Output should be attenuated
        assert!(out < 1.0);
    }
    
    #[test]
    fn test_compressor_sidechain() {
        let mut comp = Compressor::new(44100.0);
        comp.threshold = -20.0;
        comp.ratio = 4.0;
        comp.attack = 0.001;
        
        let mut out = 0.0;
        for _ in 0..100 {
            // Compress low level input signal based on a hot sidechain signal
            out = comp.process_sidechain(0.5, 1.0);
        }
        
        // Output should be attenuated below 0.5 because the sidechain signal was hot
        assert!(out < 0.5);
    }
}
