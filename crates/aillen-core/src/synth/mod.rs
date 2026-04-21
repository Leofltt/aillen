pub mod two_op;

use crate::dsp::AudioNode;

pub trait Voice: AudioNode {
    fn note_on(&mut self, frequency: f32, velocity: f32);
    fn note_off(&mut self);
    fn is_active(&self) -> bool;
    fn set_frequency(&mut self, frequency: f32);
}
