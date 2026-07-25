/// Two-Operator Synthesizer module.
pub mod two_op;
/// Sampler module.
pub mod sampler;
/// Synth303 module.
pub mod synth303;
/// Hubass synthesizer module.
pub mod hubass;

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

/// A unified interface representing a playable polyphonic synth or sampler.
pub trait PlayableInstrument: Send {
    /// Generates a single sample frame, returning a stereo pair.
    fn process(&mut self) -> (f32, f32);
    /// Triggers note playback.
    fn note_on(&mut self, frequency: f32, velocity: f32);
    /// Releases active note(s) at the specified frequency.
    fn note_off(&mut self, frequency: f32);
    /// Releases all active notes immediately.
    fn note_off_all(&mut self);
    /// Returns a reference to Any for downcasting.
    fn as_any(&self) -> &dyn std::any::Any;
    /// Returns a mutable reference to Any for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}
