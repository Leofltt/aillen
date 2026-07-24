use crate::dsp::AudioProcessor;
use std::f32::consts::PI;

/// Sources of modulation for the AM / Ring Modulator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModulationSource {
    /// Internal sine wave oscillator.
    Sine,
    /// The input signal itself (modulates input with input).
    SelfMod,
    /// An external sidechain signal.
    Sidechain,
}

/// A sidechainable Amplitude / Ring Modulator processor.
pub struct AmRingMod {
    sample_rate: f32,
    /// The source used for modulation.
    pub source: ModulationSource,
    /// Modulator frequency in Hz (only used if source is `Sine`).
    pub frequency: f32,
    /// Modulation depth/amount (range 0.0 to 1.0).
    pub depth: f32,
    /// If true, performs Ring Modulation (Carrier * Modulator).
    /// If false, performs Amplitude Modulation (Carrier * (1.0 + Depth * Modulator)).
    pub ring_mod: bool,
    
    // Internal oscillator state
    phase: f32,
}

impl AmRingMod {
    /// Creates a new `AmRingMod` instance with default settings:
    /// - source: `Sine`
    /// - frequency: 440.0 Hz
    /// - depth: 0.0 (off by default)
    /// - ring_mod: true (Ring Mod mode)
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            source: ModulationSource::Sine,
            frequency: 440.0,
            depth: 0.0,
            ring_mod: true,
            phase: 0.0,
        }
    }

    /// Processes a single frame of audio with an external sidechain signal.
    pub fn process_sidechain(&mut self, input: f32, sidechain: f32) -> f32 {
        // Get modulator signal in range [-1.0, 1.0] (or similar based on input)
        let modulator = match self.source {
            ModulationSource::Sine => {
                let val = self.phase.sin();
                let phase_step = (2.0 * PI * self.frequency) / self.sample_rate;
                self.phase = (self.phase + phase_step) % (2.0 * PI);
                val
            }
            ModulationSource::SelfMod => {
                input
            }
            ModulationSource::Sidechain => {
                sidechain
            }
        };

        // Apply modulation based on mode and depth
        let out = if self.ring_mod {
            // Ring Modulation: output = input * (1.0 - depth) + (input * modulator) * depth
            let wet = input * modulator;
            input * (1.0 - self.depth) + wet * self.depth
        } else {
            // Amplitude Modulation: output = input * (1.0 + depth * modulator)
            input * (1.0 + self.depth * modulator)
        };

        // Prevent denormals
        if out.abs() < 1e-15 { 0.0 } else { out }
    }
}

impl AudioProcessor for AmRingMod {
    /// Processes a single mono frame of audio using the selected modulator source (falls back to 0.0 for sidechain if none is provided).
    fn process(&mut self, input: f32) -> f32 {
        self.process_sidechain(input, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_mod_sine() {
        let mut arm = AmRingMod::new(44100.0);
        arm.source = ModulationSource::Sine;
        arm.frequency = 1000.0;
        arm.depth = 1.0;
        arm.ring_mod = true;

        // Process a few samples
        let out = arm.process(1.0);
        // Phase starts at 0, sin(0) is 0, ring mod depth 1.0 -> input * 0.0 = 0.0
        assert!((out - 0.0).abs() < 1e-5);

        // Next sample will have advanced phase
        let out2 = arm.process(1.0);
        assert!(out2 != 0.0);
    }

    #[test]
    fn test_ring_mod_depth_zero() {
        let mut arm = AmRingMod::new(44100.0);
        arm.source = ModulationSource::Sine;
        arm.depth = 0.0; // No effect
        arm.ring_mod = true;

        let out = arm.process(0.75);
        assert!((out - 0.75).abs() < 1e-5);
    }

    #[test]
    fn test_am_modulation() {
        let mut arm = AmRingMod::new(44100.0);
        arm.source = ModulationSource::Sine;
        arm.depth = 0.5;
        arm.ring_mod = false; // AM mode

        // Phase starts at 0, modulator is 0.0. AM is: input * (1.0 + depth * 0.0) = input
        let out = arm.process(0.8);
        assert!((out - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_ring_mod_self() {
        let mut arm = AmRingMod::new(44100.0);
        arm.source = ModulationSource::SelfMod;
        arm.depth = 1.0;
        arm.ring_mod = true;

        // self mod ring modulation: input * input
        let out = arm.process(0.5);
        assert!((out - 0.25).abs() < 1e-5);
    }

    #[test]
    fn test_ring_mod_sidechain() {
        let mut arm = AmRingMod::new(44100.0);
        arm.source = ModulationSource::Sidechain;
        arm.depth = 1.0;
        arm.ring_mod = true;

        // sidechain ring modulation: input * sidechain
        let out = arm.process_sidechain(0.5, 0.8);
        assert!((out - 0.4).abs() < 1e-5);
    }
}
