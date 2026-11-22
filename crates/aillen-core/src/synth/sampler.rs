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
    /// Monotonic trigger counter for oldest-voice stealing.
    voice_counter: u64,
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
            voice_counter: 0,
        }
    }

    /// Sets the underlying shared audio sample buffer for subsequent notes
    /// without disrupting already-sounding voices.
    pub fn set_sample_arc(&mut self, buffer: Arc<SampleBuffer>) {
        self.sample_buffer = Some(buffer);
    }

    /// Sets the underlying audio sample buffer.
    pub fn set_sample(&mut self, buffer: SampleBuffer) {
        self.set_sample_arc(Arc::new(buffer));
    }

    /// Configures the sampler playback mode.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
        for voice in &mut self.voices {
            if voice.active {
                voice.play_mode = mode;
            }
        }
    }

    /// Configures the pitch ratio.
    pub fn set_pitch_ratio(&mut self, ratio: f32) {
        self.pitch_ratio = if ratio.is_finite() {
            ratio.clamp(0.001, 16.0)
        } else {
            1.0
        };
        let root = self.root_freq.max(1e-6);
        for voice in &mut self.voices {
            if voice.active {
                voice.pitch_ratio = self.pitch_ratio * (voice.triggered_freq / root);
            }
        }
    }

    /// Configures the playback speed ratio.
    pub fn set_speed_ratio(&mut self, ratio: f32) {
        self.speed_ratio = if ratio.is_finite() {
            ratio.clamp(0.001, 16.0)
        } else {
            1.0
        };
        for voice in &mut self.voices {
            if voice.active {
                voice.speed_ratio = self.speed_ratio;
            }
        }
    }

    /// Configures the time-stretch engine mode.
    pub fn set_stretch_mode(&mut self, mode: StretchMode) {
        self.stretch_mode = mode;
        for voice in &mut self.voices {
            if voice.active {
                voice.stretch_mode = mode;
            }
        }
    }

    /// Configures the grain duration in milliseconds.
    pub fn set_grain_size(&mut self, size_ms: f32) {
        self.grain_size_ms = if size_ms.is_finite() {
            size_ms.clamp(5.0, 500.0)
        } else {
            40.0
        };
        for voice in &mut self.voices {
            if voice.active {
                voice.grain_size_ms = self.grain_size_ms;
            }
        }
    }

    /// Configures the grain overlap factor.
    pub fn set_overlap(&mut self, overlap: usize) {
        self.overlap = overlap.clamp(1, 16);
        for voice in &mut self.voices {
            if voice.active {
                voice.overlap = self.overlap;
            }
        }
    }

    /// Enables or disables slicing mode (takes effect on the next `note_on`).
    pub fn set_slice_mode(&mut self, enabled: bool) {
        self.slice_mode = enabled;
    }

    /// Sets the total number of slices (takes effect on the next `note_on`).
    pub fn set_num_slices(&mut self, n: usize) {
        self.num_slices = n.clamp(1, 128);
    }

    /// Sets the selected slice index (takes effect on the next `note_on`).
    pub fn set_selected_slice(&mut self, slice: usize) {
        self.selected_slice = slice;
    }

    /// Sets the stutter count (takes effect on the next `note_on`).
    pub fn set_stutter_count(&mut self, count: usize) {
        self.stutter_count = count.clamp(1, 64);
    }

    /// Estimates the playback duration of a triggered note/slice in milliseconds,
    /// accounting for slicing mode, stretch mode, playback speed, and pitch ratio.
    pub fn estimated_note_duration_ms(&self, frequency: f32) -> Option<f32> {
        let buffer = self.sample_buffer.as_ref()?;
        if buffer.channels == 0 || buffer.sample_rate <= 0.0 {
            return None;
        }
        let num_frames = buffer.data.len() / buffer.channels;
        if num_frames == 0 {
            return None;
        }

        let frames_to_play = if self.slice_mode {
            (num_frames as f64) / (self.num_slices.max(1) as f64)
        } else {
            num_frames as f64
        };

        let rate_multiplier = match self.stretch_mode {
            StretchMode::Resample => {
                let effective_pitch = self.pitch_ratio * (frequency / self.root_freq.max(1e-6));
                (effective_pitch * self.speed_ratio).max(0.001) as f64
            }
            StretchMode::Granular => self.speed_ratio.max(0.001) as f64,
        };

        let duration_sec = frames_to_play / (buffer.sample_rate as f64 * rate_multiplier);
        Some((duration_sec * 1000.0).max(1.0) as f32)
    }

    /// Triggers note playback. If `slice_mode` is enabled, smoothly chokes any
    /// active voices (~2 ms fade-out). Otherwise runs polyphonically and steals
    /// the oldest voice only when all voices are busy.
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        let Some(buffer) = self.sample_buffer.clone() else {
            return;
        };
        if self.voices.is_empty() {
            return;
        }

        if self.slice_mode {
            for voice in &mut self.voices {
                if voice.active {
                    voice.begin_fade_out();
                }
            }
        }

        let target_idx = self
            .voices
            .iter()
            .position(|v| !v.active)
            .or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.is_fading_out())
                    .min_by_key(|(_, v)| v.age)
                    .map(|(idx, _)| idx)
            })
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map(|(idx, _)| idx)
                    .unwrap_or(0)
            });

        self.voice_counter = self.voice_counter.wrapping_add(1);
        let root = self.root_freq.max(1e-6);
        let voice = &mut self.voices[target_idx];
        voice.age = self.voice_counter;
        voice.set_sample(buffer);
        voice.play_mode = self.play_mode;
        voice.speed_ratio = self.speed_ratio;
        voice.pitch_ratio = self.pitch_ratio * (frequency / root);
        voice.stretch_mode = self.stretch_mode;
        voice.grain_size_ms = self.grain_size_ms;
        voice.overlap = self.overlap;
        voice.slice_mode = self.slice_mode;
        voice.num_slices = self.num_slices;
        voice.selected_slice = self.selected_slice;
        voice.stutter_count = self.stutter_count;
        voice.note_on(frequency, velocity);
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
        let mut sustained_count = 0;

        for voice in &mut self.voices {
            if voice.active {
                let fading = voice.is_fading_out();
                let (l, r) = voice.process();
                mix_l += l;
                mix_r += r;
                if !fading {
                    sustained_count += 1;
                }
            }
        }

        let headroom = 1.0 / (sustained_count as f32).max(1.0).sqrt();
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
        voice.stutter_count = 4;

        let mut data = vec![0.5; 10000];
        data[2500] = 0.0; // Anchor zero-crossing exactly at slice 1 start
        let buffer = SampleBuffer {
            data,
            channels: 1,
            sample_rate: 44100.0,
        };
        voice.set_sample(Arc::new(buffer));
        voice.note_on(261.63, 1.0);

        assert!(voice.active);

        let (l0, r0) = voice.process();
        assert_eq!(l0, 0.0);
        assert_eq!(r0, 0.0);

        let mut max_abs = 0.0_f32;
        let mut peak_count = 0_usize;
        let mut in_peak = false;
        let mut total_samples = 1_usize;

        while voice.active && total_samples < 5000 {
            let (l, r) = voice.process();
            max_abs = max_abs.max(l.abs()).max(r.abs());
            if l > 0.45 && !in_peak {
                peak_count += 1;
                in_peak = true;
            } else if l < 0.05 {
                in_peak = false;
            }
            total_samples += 1;
        }

        assert!(!voice.active, "stuttered slice should deactivate cleanly after one slice duration");
        assert!(max_abs <= 0.5001, "stutter volume exploded: {max_abs}");
        assert_eq!(peak_count, 4, "expected 4 distinct stutter repeats, got {peak_count}");
        assert!(total_samples <= 2505, "expected total length ~2500 samples, got {total_samples}");
    }

    #[test]
    fn test_sampler_granular_overlap_and_oneshot() {
        let mut voice = SamplerVoice::new(44100.0);
        voice.stretch_mode = StretchMode::Granular;
        voice.grain_size_ms = 20.0;
        voice.overlap = 8;
        voice.pitch_ratio = 2.0;

        let buffer = SampleBuffer {
            data: vec![1.0; 2000],
            channels: 1,
            sample_rate: 44100.0,
        };
        voice.set_sample(Arc::new(buffer));
        voice.note_on(261.63, 1.0);

        let mut max_abs = 0.0_f32;
        for _ in 0..3500 {
            let (l, r) = voice.process();
            max_abs = max_abs.max(l.abs()).max(r.abs());
        }

        assert!(max_abs > 0.5, "granular mode produced insufficient level: {max_abs}");
        assert!(max_abs <= 1.15, "granular overlap gain not normalized: {max_abs}");
        assert!(!voice.active, "one-shot granular voice did not finish after EOF + grain tail");
    }

    #[test]
    fn test_sampler_slice_choke_and_polyphony() {
        let mut sliced = Sampler::new(44100.0, 4);
        sliced.set_slice_mode(true);
        sliced.set_num_slices(4);
        sliced.set_sample(SampleBuffer {
            data: vec![0.5; 10000],
            channels: 1,
            sample_rate: 44100.0,
        });
        sliced.note_on(261.63, 1.0);
        let mut last = (0.0, 0.0);
        for _ in 0..200 {
            last = sliced.process();
        }
        assert!((last.0 - 0.5).abs() < 1e-4, "expected sustained level 0.5, got {}", last.0);

        // Second note in slice_mode smoothly chokes previous voice within ~3 ms
        sliced.note_on(261.63, 1.0);
        for _ in 0..140 {
            sliced.process();
        }
        assert_eq!(sliced.voices.iter().filter(|v| v.active).count(), 1);

        // Non-sliced sampler stays polyphonic and isolates active voices from new buffers/params
        let mut poly = Sampler::new(44100.0, 4);
        poly.set_sample(SampleBuffer {
            data: vec![0.4; 2000],
            channels: 1,
            sample_rate: 44100.0,
        });
        poly.note_on(261.63, 1.0);
        for _ in 0..50 {
            poly.process();
        }

        poly.set_sample(SampleBuffer {
            data: vec![0.8; 2000],
            channels: 1,
            sample_rate: 44100.0,
        });
        poly.set_stutter_count(4);
        poly.note_on(261.63, 1.0);

        assert_eq!(poly.voices.iter().filter(|v| v.active).count(), 2);
        assert_eq!(poly.voices[0].stutter_count, 1);
        assert_eq!(poly.voices[0].sample_buffer.as_ref().unwrap().data[0], 0.4);
        assert_eq!(poly.voices[1].stutter_count, 4);
        assert_eq!(poly.voices[1].sample_buffer.as_ref().unwrap().data[0], 0.8);
    }
}
