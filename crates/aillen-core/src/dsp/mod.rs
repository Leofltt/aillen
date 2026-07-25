pub mod oscillator;
pub mod filter;
pub mod envelope;
pub mod panner;
pub mod compressor;
pub mod am_ring_mod;
pub mod delay;
pub mod fx_chain;
pub mod waveloss;
pub mod lfo;
pub mod distortion;

pub use compressor::Compressor;
pub use am_ring_mod::{AmRingMod, ModulationSource};
pub use delay::{StereoDelay, DelayMode};
pub use fx_chain::FxChain;
pub use waveloss::WaveLoss;

/// A trait for generating or processing a single frame of audio data.
pub trait AudioNode {
    fn process(&mut self) -> f32;
}

/// A trait for processing audio with an input.
pub trait AudioProcessor {
    fn process(&mut self, input: f32) -> f32;
}

/// A trait for processing audio with stereo input/output.
pub trait StereoProcessor {
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32);
    /// Optionally process with stereo sidechain modulation inputs. Defaults to ignoring the modulation.
    fn process_stereo_modulated(&mut self, left: f32, right: f32, _mod_l: f32, _mod_r: f32) -> (f32, f32) {
        self.process_stereo(left, right)
    }
}
