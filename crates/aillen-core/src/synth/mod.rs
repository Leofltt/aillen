/// Two-Operator Synthesizer module.
pub mod two_op;
/// Sampler module.
pub mod sampler;

use crate::dsp::AudioNode;

/// Represents a single active audio voice inside a synthesizer.
pub trait Voice: AudioNode {
    /// Triggers note playback at the specified frequency and velocity.
    fn note_on(&mut self, frequency: f32, velocity: f32);
    /// Releases the active note.
    fn note_off(&mut self);
    /// Checks if the voice is currently producing sound.
    fn is_active(&self) -> bool;
    /// Dynamically shifts the active note's frequency.
    fn set_frequency(&mut self, frequency: f32);
}
