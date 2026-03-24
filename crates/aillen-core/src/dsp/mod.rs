pub mod oscillator;
pub mod filter;
pub mod envelope;

/// A trait for generating or processing a single frame of audio data.
pub trait AudioNode {
    fn process(&mut self) -> f32;
}

/// A trait for processing audio with an input.
pub trait AudioProcessor {
    fn process(&mut self, input: f32) -> f32;
}
