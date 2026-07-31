pub mod buffer;
pub mod voice;

pub use buffer::{SampleBuffer, PlayMode, StretchMode, load_audio_file};
pub use voice::{Grain, SamplerVoice};

use std::sync::Arc;
use crate::dsp::filter::DjFilter;
use crate::dsp::AudioProcessor;

/// The multi-voice sampler manager loaded as an Instrument.
pub struct Sampler {
    /// Array of voices available for polyphony.
    pub voices: Vec<SamplerVoice>,
    /// Shared audio sample buffer.
    pub sample_buffer: Option<Arc<SampleBuffer>>,
    /// Playback loop or one-shot mode.
    pub play_mode: PlayMode,
    /// Pitch shifting factor.
    pub pitch_ratio: f32,
    /// Playback speed factor.
    pub speed_ratio: f32,
    /// MIDI note reference frequency (root key).
    pub root_freq: f32,

    /// Time stretch engine mode.
    pub stretch_mode: StretchMode,
    /// Grain duration size in milliseconds.
    pub grain_size_ms: f32,
    /// Overlapping grain count.
    pub overlap: usize,
    /// Stereo DJ performance filter left channel.
    pub dj_filter_l: DjFilter,
    /// Stereo DJ performance filter right channel.
    pub dj_filter_r: DjFilter,

    /// Slicing mode active state.
    pub slice_mode: bool,
    /// Total slices to divide the buffer into.
    pub num_slices: usize,
    /// Currently selected slice index.
    pub selected_slice: usize,
    /// Stutter count repeats.
    pub stutter_count: usize,
    /// File path of the currently loaded sample.
    pub current_path: Option<String>,
}

impl Sampler {
    /// Instantiates a new Sampler containing `num_voices` polyphonic voices.
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            voices.push(SamplerVoice::new(sample_rate));
        }
        Self {
            voices,
            sample_buffer: None,
            play_mode: PlayMode::OneShot,
            pitch_ratio: 1.0,
            speed_ratio: 1.0,
            root_freq: 261.63,
            stretch_mode: StretchMode::Resample,
            grain_size_ms: 40.0,
            overlap: 4,
            dj_filter_l: DjFilter::new(sample_rate),
            dj_filter_r: DjFilter::new(sample_rate),
            slice_mode: false,
            num_slices: 16,
            selected_slice: 0,
            stutter_count: 1,
            current_path: None,
        }
    }

    /// Sets the underlying audio sample buffer.
    pub fn set_sample(&mut self, buffer: SampleBuffer) {
        let arc_buf = Arc::new(buffer);
        self.sample_buffer = Some(arc_buf.clone());
        for voice in &mut self.voices {
            voice.set_sample(arc_buf.clone());
        }
    }

    /// Configures the sampler playback mode.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
        for voice in &mut self.voices {
            voice.play_mode = mode;
        }
    }

    /// Configures the pitch ratio.
    pub fn set_pitch_ratio(&mut self, ratio: f32) {
        self.pitch_ratio = ratio.max(0.001);
        for voice in &mut self.voices {
            voice.pitch_ratio = self.pitch_ratio * (voice.triggered_freq / self.root_freq);
        }
    }

    /// Configures the playback speed ratio.
    pub fn set_speed_ratio(&mut self, ratio: f32) {
        self.speed_ratio = ratio.max(0.001);
        for voice in &mut self.voices {
            voice.speed_ratio = self.speed_ratio;
        }
    }

    /// Configures the time-stretch engine mode.
    pub fn set_stretch_mode(&mut self, mode: StretchMode) {
        self.stretch_mode = mode;
        for voice in &mut self.voices {
            voice.stretch_mode = mode;
        }
    }

    /// Configures the grain duration in milliseconds.
    pub fn set_grain_size(&mut self, size_ms: f32) {
        self.grain_size_ms = size_ms.clamp(5.0, 500.0);
        for voice in &mut self.voices {
            voice.grain_size_ms = self.grain_size_ms;
        }
    }

    /// Configures the grain overlap factor.
    pub fn set_overlap(&mut self, overlap: usize) {
        self.overlap = overlap.clamp(1, 16);
        for voice in &mut self.voices {
            voice.overlap = self.overlap;
        }
    }

    /// Enables or disables slicing mode.
    pub fn set_slice_mode(&mut self, enabled: bool) {
        self.slice_mode = enabled;
        for voice in &mut self.voices {
            voice.slice_mode = enabled;
        }
    }

    /// Sets the total number of slices.
    pub fn set_num_slices(&mut self, n: usize) {
        self.num_slices = n.max(1);
        for voice in &mut self.voices {
            voice.num_slices = self.num_slices;
        }
    }

    /// Sets the selected slice index.
    pub fn set_selected_slice(&mut self, slice: usize) {
        self.selected_slice = slice;
        for voice in &mut self.voices {
            voice.selected_slice = slice;
        }
    }

    /// Sets the stutter count.
    pub fn set_stutter_count(&mut self, count: usize) {
        self.stutter_count = count.max(1);
        for voice in &mut self.voices {
            voice.stutter_count = self.stutter_count;
        }
    }

    /// Triggers note playback. Attempts to find a free voice or steals voice 0.
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.active) {
            voice.play_mode = self.play_mode;
            voice.speed_ratio = self.speed_ratio;
            voice.pitch_ratio = self.pitch_ratio * (frequency / self.root_freq);
            voice.stretch_mode = self.stretch_mode;
            voice.grain_size_ms = self.grain_size_ms;
            voice.overlap = self.overlap;
            voice.slice_mode = self.slice_mode;
            voice.num_slices = self.num_slices;
            voice.selected_slice = self.selected_slice;
            voice.stutter_count = self.stutter_count;
            voice.note_on(frequency, velocity);
        } else {
            let voice = &mut self.voices[0];
            voice.play_mode = self.play_mode;
            voice.speed_ratio = self.speed_ratio;
            voice.pitch_ratio = self.pitch_ratio * (frequency / self.root_freq);
            voice.stretch_mode = self.stretch_mode;
            voice.grain_size_ms = self.grain_size_ms;
            voice.overlap = self.overlap;
            voice.slice_mode = self.slice_mode;
            voice.num_slices = self.num_slices;
            voice.selected_slice = self.selected_slice;
            voice.stutter_count = self.stutter_count;
            voice.note_on(frequency, velocity);
        }
    }

    /// Releases active note frequencies matching `frequency`.
    pub fn note_off(&mut self, frequency: f32) {
        for voice in &mut self.voices {
            if (voice.triggered_freq - frequency).abs() < 0.01 && voice.active {
                voice.note_off();
            }
        }
    }

    /// Immediately silences all active sampler voices.
    pub fn note_off_all(&mut self) {
        for voice in &mut self.voices {
            voice.active = false;
        }
    }

    /// Sets the position of the DJ filter at the end of the sampler chain.
    pub fn set_dj_filter_position(&mut self, pos: f32) {
        self.dj_filter_l.set_position(pos);
        self.dj_filter_r.set_position(pos);
    }

    /// Sums polyphonic voice outputs, applies headroom gain, and routes through the DJ performance filter.
    pub fn process(&mut self) -> (f32, f32) {
        let mut mix_l = 0.0;
        let mut mix_r = 0.0;
        let mut active_count = 0;
        
        for voice in &mut self.voices {
            if voice.active {
                let (l, r) = voice.process();
                mix_l += l;
                mix_r += r;
                active_count += 1;
            }
        }

        let headroom = 1.0 / (active_count as f32).max(1.0).sqrt();
        let out_l = self.dj_filter_l.process(mix_l * headroom);
        let out_r = self.dj_filter_r.process(mix_r * headroom);
        (out_l, out_r)
    }
}

impl crate::synth::PlayableInstrument for Sampler {
    fn process(&mut self) -> (f32, f32) {
        self.process()
    }

    fn note_on(&mut self, frequency: f32, velocity: f32) {
        self.note_on(frequency, velocity);
    }

    fn note_off(&mut self, frequency: f32) {
        self.note_off(frequency);
    }

    fn note_off_all(&mut self) {
        self.note_off_all();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_sampler_voice_oneshot() {
        let mut voice = SamplerVoice::new(44100.0);
        let buffer = SampleBuffer {
            data: vec![0.5, -0.5, 0.25, -0.25],
            channels: 1,
            sample_rate: 44100.0,
        };
        voice.set_sample(Arc::new(buffer));
        voice.note_on(261.63, 1.0);
        
        assert!(voice.active);

        let (l1, r1) = voice.process();
        assert_eq!(l1, 0.5);
        assert_eq!(r1, 0.5);

        let (l2, r2) = voice.process();
        assert_eq!(l2, -0.5);
        assert_eq!(r2, -0.5);
    }

    #[test]
    fn test_sampler_voice_granular() {
        let mut voice = SamplerVoice::new(44100.0);
        voice.stretch_mode = StretchMode::Granular;
        voice.grain_size_ms = 10.0;
        voice.overlap = 2;

        let buffer = SampleBuffer {
            data: vec![1.0; 1000],
            channels: 1,
            sample_rate: 44100.0,
        };
        voice.set_sample(Arc::new(buffer));
        voice.note_on(261.63, 1.0);

        assert!(voice.active);

        let mut produced_audio = false;
        for _ in 0..10 {
            let (l, r) = voice.process();
            if l > 0.0 && r > 0.0 {
                produced_audio = true;
                break;
            }
        }
        assert!(produced_audio);
    }

    #[test]
    fn test_sampler_slice_stutter() {
        let mut voice = SamplerVoice::new(44100.0);
        voice.slice_mode = true;
        voice.num_slices = 4;
        voice.selected_slice = 1;
        voice.stutter_count = 2;

        let buffer = SampleBuffer {
            data: vec![0.5; 10000],
            channels: 1,
            sample_rate: 44100.0,
        };
        voice.set_sample(Arc::new(buffer));
        voice.note_on(261.63, 1.0);

        assert!(voice.active);
        
        let (l, r) = voice.process();
        assert_eq!(l, 0.0); // Faded in to 0.0 at the boundary start
        assert_eq!(r, 0.0);

        // Advance past the 5ms fade-in window (220.5 samples at 44.1kHz)
        for _ in 0..250 {
            voice.process();
        }
        let (l, r) = voice.process();
        assert_eq!(l, 0.5); // Fully faded in
        assert_eq!(r, 0.5);
    }
}
