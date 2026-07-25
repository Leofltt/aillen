/// Naive oscillator module.
pub mod naive;
/// PolyBLEP anti-aliased oscillator module.
pub mod polyblep;
/// Sub-bass oscillator module.
pub mod sub;
/// Detuned unison multi-voice oscillator engine.
pub mod unison;

// Import types for public re-export
pub use naive::NaiveOscillator;
pub use polyblep::PolyBlepOscillator;
pub use sub::{SubOscillator, SubWaveform};
pub use unison::UnisonEngine;

/// Supported oscillator waveforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Triangle,
}
