use crate::synth::PlayableInstrument;
use crate::synth::resonator::voice::{ResonatorVoice, ResonatorPatch};
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
        let mut sum_l = 0.0;
        let mut sum_r = 0.0;
        let mut active_count = 0usize;
        for voice in &mut self.voices {
            if voice.is_active() {
                let (l, r) = voice.process_stereo();
                sum_l += l;
                sum_r += r;
                active_count += 1;
            }
        }
        let headroom = 1.0 / (active_count as f32).max(1.0).sqrt();
        (sum_l * headroom, sum_r * headroom)
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

    #[test]
    fn test_synth_resonator_modal_mix_sustain_and_loudness() {
        let mut synth = SynthResonator::new(44100.0, 4);
        let mut patch = synth.patch.clone();
        patch.modal_mix = 1.0;
        patch.modal_ratio = 2.76;
        patch.feedback = 0.95;
        synth.set_patch(patch);

        synth.note_on(220.0, 1.0);
        let mut sum_sq = 0.0;
        let num_samples = 11025; // 250 ms
        for _ in 0..num_samples {
            let (l, _) = synth.process();
            sum_sq += l * l;
        }
        let rms = (sum_sq / num_samples as f32).sqrt();
        assert!(synth.voices[0].is_active(), "voice cut out prematurely");
        assert!(rms > 0.04, "modal resonator RMS too quiet: {}", rms);
    }

    #[test]
    fn test_synth_resonator_modal_width_stereo() {
        let mut synth = SynthResonator::new(44100.0, 4);
        let mut patch = synth.patch.clone();
        patch.modal_mix = 1.0;
        patch.modal_ratio = 2.76;
        patch.modal_width = 0.0;
        synth.set_patch(patch.clone());

        synth.note_on(220.0, 1.0);
        for _ in 0..500 {
            let (l, r) = synth.process();
            assert!((l - r).abs() < 1e-6);
        }

        patch.modal_width = 1.0;
        synth.set_patch(patch);
        synth.note_on(220.0, 1.0);
        let mut max_diff = 0.0_f32;
        for _ in 0..500 {
            let (l, r) = synth.process();
            max_diff = max_diff.max((l - r).abs());
        }
        assert!(max_diff > 1e-2, "expected stereo spread, max diff = {}", max_diff);
    }

    #[test]
    fn test_synth_resonator_velocity_scaling() {
        let mut soft = SynthResonator::new(44100.0, 1);
        soft.note_on(220.0, 0.25);
        let mut loud = SynthResonator::new(44100.0, 1);
        loud.note_on(220.0, 1.0);

        let mut soft_energy = 0.0;
        let mut loud_energy = 0.0;
        for _ in 0..2000 {
            let (sl, _) = soft.process();
            let (ll, _) = loud.process();
            soft_energy += sl * sl;
            loud_energy += ll * ll;
        }
        assert!(soft_energy < loud_energy * 0.5);
    }
}
