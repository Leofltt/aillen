use crate::synth::PlayableInstrument;
use crate::synth::resonator::voice::{ResonatorVoice, ResonatorPatch};
use crate::dsp::AudioNode;
use std::any::Any;


/// Polyphonic Circuit-Bent Karplus-Strong Resonator Synthesizer.
pub struct SynthResonator {
    pub sample_rate: f32,
    pub voices: Vec<ResonatorVoice>,
    pub patch: ResonatorPatch,
}

impl SynthResonator {
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let patch = ResonatorPatch::default();
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            let mut v = ResonatorVoice::new(sample_rate);
            v.set_patch(patch.clone());
            voices.push(v);
        }

        Self {
            sample_rate,
            voices,
            patch,
        }
    }
    pub fn set_patch(&mut self, patch: ResonatorPatch) {
        self.patch = patch;
        for voice in &mut self.voices {
            voice.set_patch(self.patch.clone());
        }
    }

    pub fn trigger_note(&mut self, frequency: f32, velocity: f32, duration_ms: f32) {
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
            voice.trigger_note(frequency, velocity, duration_ms);
        } else if let Some(voice) = self.voices.first_mut() {
            voice.trigger_note(frequency, velocity, duration_ms);
        }
    }
}



impl PlayableInstrument for SynthResonator {
    fn process(&mut self) -> (f32, f32) {
        let mut sum = 0.0;
        for voice in &mut self.voices {
            if voice.is_active() {
                sum += voice.process();
            }
        }
        (sum, sum)
    }

    fn note_on(&mut self, frequency: f32, velocity: f32) {
        // Find inactive voice, or steal oldest/first voice
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.is_active()) {
            voice.note_on(frequency, velocity);
        } else if let Some(voice) = self.voices.first_mut() {
            voice.note_on(frequency, velocity);
        }
    }

    fn note_off(&mut self, frequency: f32) {
        for voice in &mut self.voices {
            if voice.is_active() && (voice.current_frequency - frequency).abs() < 0.5 {
                voice.note_off();
            }
        }
    }

    fn note_off_all(&mut self) {
        for voice in &mut self.voices {
            voice.note_off();
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_resonator_note_trigger() {
        let mut synth = SynthResonator::new(44100.0, 4);
        synth.note_on(440.0, 1.0);

        let (l, r) = synth.process();
        assert_ne!(l, 0.0);
        assert_ne!(r, 0.0);
    }
}
