use std::sync::Arc;
use super::buffer::{SampleBuffer, PlayMode, StretchMode};

/// Represents an individual active sound grain in the granular engine.
#[derive(Clone, Copy, Debug, Default)]
pub struct Grain {
    /// The starting index in the source buffer.
    pub source_start: f64,
    /// The current phase offset in output samples within the grain.
    pub phase: f64,
    /// Total grain lifetime in output samples.
    pub duration: f64,
    /// Whether this grain is active.
    pub active: bool,
}

/// Represents a single polyphonic voice playing back a sample buffer.
pub struct SamplerVoice {
    /// Audio stream sample rate.
    pub sample_rate: f32,
    /// Reference-counted shared sample buffer.
    pub sample_buffer: Option<Arc<SampleBuffer>>,
    /// Whether the voice is active.
    pub active: bool,
    /// Monotonic trigger sequence counter used for oldest-voice stealing.
    pub age: u64,
    /// Playback mode (Loop or OneShot).
    pub play_mode: PlayMode,
    /// Current pitch shifting factor.
    pub pitch_ratio: f32,
    /// Current playback speed factor.
    pub speed_ratio: f32,
    /// Gain velocity factor.
    pub velocity: f32,
    /// Triggered MIDI note frequency.
    pub triggered_freq: f32,
    /// Main playback position index.
    pub phase: f64,

    /// Playback stretch method (Resample or Granular).
    pub stretch_mode: StretchMode,
    /// Size of grains in milliseconds (e.g. 40.0 ms).
    pub grain_size_ms: f32,
    /// Number of active overlapping grains (e.g. 4).
    pub overlap: usize,
    /// Fixed array storing active grain data.
    pub grains: [Grain; 16],
    /// Timer determining when to spawn the next grain.
    pub grain_spawn_timer: f32,
    /// Linear congruential generator state.
    pub rng_seed: u32,

    /// Whether slicing mode is enabled.
    pub slice_mode: bool,
    /// Total number of slices (e.g. 16).
    pub num_slices: usize,
    /// Selected active slice index.
    pub selected_slice: usize,
    /// Number of stutter repeats.
    pub stutter_count: usize,

    // Internal slice boundaries
    pub slice_start: f64,
    pub sub_slice_len: f64,
    pub stutter_index: usize,

    // Smooth anti-click choke / note-off release envelope
    fade_out_gain: f32,
    fade_out_step: f32,
}

impl SamplerVoice {
    /// Creates a new SamplerVoice configured for the target sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            sample_buffer: None,
            active: false,
            age: 0,
            play_mode: PlayMode::OneShot,
            pitch_ratio: 1.0,
            speed_ratio: 1.0,
            velocity: 0.0,
            triggered_freq: 0.0,
            phase: 0.0,
            stretch_mode: StretchMode::Resample,
            grain_size_ms: 40.0,
            overlap: 4,
            grains: [Grain::default(); 16],
            grain_spawn_timer: 999999.0, // Force spawn immediately on start
            rng_seed: 123456789,
            slice_mode: false,
            num_slices: 16,
            selected_slice: 0,
            stutter_count: 1,
            slice_start: 0.0,
            sub_slice_len: 0.0,
            stutter_index: 0,
            fade_out_gain: 1.0,
            fade_out_step: 0.0,
        }
    }

    /// Initiates a smooth 2 ms fade-out (used for monophonic slice choke and loop release).
    pub fn begin_fade_out(&mut self) {
        if self.active && self.fade_out_step == 0.0 {
            let fade_samples = (0.002 * self.sample_rate.max(1.0)).max(1.0);
            self.fade_out_step = 1.0 / fade_samples;
        }
    }

    /// Returns true if this voice is currently fading out.
    pub fn is_fading_out(&self) -> bool {
        self.fade_out_step > 0.0
    }

    #[inline]
    fn apply_fade_out(&mut self, mut out_l: f32, mut out_r: f32) -> (f32, f32) {
        if self.fade_out_step > 0.0 {
            out_l *= self.fade_out_gain;
            out_r *= self.fade_out_gain;
            self.fade_out_gain -= self.fade_out_step;
            if self.fade_out_gain <= 0.0 {
                self.fade_out_gain = 0.0;
                self.active = false;
            }
        }
        (out_l, out_r)
    }

    #[inline]
    fn read_interpolated(
        data: &[f32],
        channels: usize,
        num_frames: usize,
        pos: f64,
        wrap: bool,
    ) -> (f32, f32) {
        if num_frames == 0 || channels == 0 || !pos.is_finite() {
            return (0.0, 0.0);
        }

        let (idx0, idx1, frac) = if wrap {
            let wrapped = pos.rem_euclid(num_frames as f64);
            let i0 = (wrapped.floor() as usize) % num_frames;
            let i1 = (i0 + 1) % num_frames;
            let f = (wrapped - wrapped.floor()) as f32;
            (i0, i1, f)
        } else {
            if pos < 0.0 || pos >= num_frames as f64 {
                return (0.0, 0.0);
            }
            let i0 = pos.floor() as usize;
            let i1 = i0 + 1;
            let f = (pos - i0 as f64) as f32;
            (i0, i1, f)
        };

        if channels == 1 {
            let s0 = data[idx0];
            let s1 = if idx1 < num_frames { data[idx1] } else { 0.0 };
            let sample = s0 + (s1 - s0) * frac;
            (sample, sample)
        } else {
            let base0 = idx0 * channels;
            let s0_l = data[base0];
            let s0_r = data[base0 + 1];
            let (s1_l, s1_r) = if idx1 < num_frames {
                let base1 = idx1 * channels;
                (data[base1], data[base1 + 1])
            } else {
                (0.0, 0.0)
            };
            (s0_l + (s1_l - s0_l) * frac, s0_r + (s1_r - s0_r) * frac)
        }
    }

    /// Returns a pseudo-random float between 0.0 and 1.0 using a fast LCG.
    fn next_random(&mut self) -> f32 {
        self.rng_seed = self.rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng_seed as f32) / (u32::MAX as f32)
    }

    /// Finds the closest zero-crossing point in a window around target_frame.
    fn find_closest_zero_crossing(&self, data: &[f32], target_frame: usize, channels: usize, window: usize) -> usize {
        if channels == 0 {
            return 0;
        }
        let num_frames = data.len() / channels;
        if num_frames == 0 {
            return 0;
        }

        let target = target_frame.min(num_frames - 1);
        let mut best_frame = target;
        let mut min_abs = f32::MAX;

        let start_search = target.saturating_sub(window);
        let end_search = target.saturating_add(window).min(num_frames);

        for frame in start_search..end_search {
            let mut sum_abs = 0.0;
            for c in 0..channels {
                sum_abs += data[frame * channels + c].abs();
            }
            let avg_abs = sum_abs / channels as f32;
            if avg_abs < min_abs {
                min_abs = avg_abs;
                best_frame = frame;
                if min_abs == 0.0 {
                    break;
                }
            }
        }
        best_frame
    }

    /// Sets the shared sample buffer for this voice.
    pub fn set_sample(&mut self, buffer: Arc<SampleBuffer>) {
        self.sample_buffer = Some(buffer);
    }

    /// Triggers note playback at a given frequency and velocity.
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        self.triggered_freq = frequency;
        self.velocity = velocity;
        self.phase = 0.0;
        self.stutter_index = 0;
        self.fade_out_gain = 1.0;
        self.fade_out_step = 0.0;

        for grain in &mut self.grains {
            grain.active = false;
            grain.phase = 0.0;
        }

        let mut playable = false;
        if let Some(ref buffer) = self.sample_buffer {
            let num_frames = if buffer.channels > 0 {
                buffer.data.len() / buffer.channels
            } else {
                0
            };

            if num_frames > 0 && buffer.sample_rate > 0.0 {
                playable = true;
                if self.slice_mode {
                    let slices = self.num_slices.max(1);
                    let slice_idx = self.selected_slice.min(slices - 1);
                    let slice_len = num_frames as f64 / slices as f64;
                    let target_start = ((slice_idx as f64 * slice_len) as usize).min(num_frames - 1);
                    let adjusted_start = if target_start == 0 {
                        0
                    } else {
                        self.find_closest_zero_crossing(&buffer.data, target_start, buffer.channels, 64)
                    };
                    let available = (num_frames as f64 - adjusted_start as f64).max(1.0);
                    let effective_slice_len = slice_len.min(available);
                    self.slice_start = adjusted_start as f64;
                    self.sub_slice_len = (effective_slice_len / self.stutter_count.max(1) as f64).max(1.0);
                    self.phase = self.slice_start;
                } else {
                    self.slice_start = 0.0;
                    self.sub_slice_len = num_frames as f64;
                }

                let grain_dur_out = ((self.grain_size_ms.clamp(5.0, 500.0) / 1000.0) * self.sample_rate.max(1.0)).max(4.0);
                let overlap = self.overlap.clamp(1, 16) as f32;
                self.grain_spawn_timer = grain_dur_out / overlap;
            }
        }

        self.active = playable;
    }

    /// Releases the active note.
    pub fn note_off(&mut self) {
        if self.play_mode == PlayMode::Loop {
            self.begin_fade_out();
        }
    }

    /// Generates a single stereo frame of sample playback.
    pub fn process(&mut self) -> (f32, f32) {
        let buffer = match &self.sample_buffer {
            Some(b) if self.active => Arc::clone(b),
            _ => return (0.0, 0.0),
        };

        let data = &buffer.data;
        let channels = buffer.channels;
        let original_sample_rate = buffer.sample_rate;
        let num_frames = if channels > 0 { data.len() / channels } else { 0 };

        if channels == 0 || num_frames == 0 || original_sample_rate <= 0.0 {
            self.active = false;
            return (0.0, 0.0);
        }

        match self.stretch_mode {
            StretchMode::Resample => {
                let rate_multiplier = (self.pitch_ratio * self.speed_ratio).max(0.0);
                let phase_increment = (original_sample_rate as f64 / self.sample_rate.max(1.0) as f64) * rate_multiplier as f64;

                let index = self.phase;
                let wrap_read = !self.slice_mode && self.play_mode == PlayMode::Loop;
                let (raw_l, raw_r) = Self::read_interpolated(data, channels, num_frames, index, wrap_read);
                let mut out_l = raw_l * self.velocity;
                let mut out_r = raw_r * self.velocity;

                let sub_slice_start = self.slice_start.clamp(0.0, (num_frames - 1) as f64);
                let sub_slice_end = (sub_slice_start + self.sub_slice_len)
                    .min(num_frames as f64)
                    .max(sub_slice_start + 1.0);
                let effective_len = (sub_slice_end - sub_slice_start).max(1.0);

                if self.slice_mode {
                    let relative_phase = (index - sub_slice_start).max(0.0);
                    let dist_from_end = (sub_slice_end - index).max(0.0);

                    // 1.5 ms smoothstep fade window, capped at half the sub-slice length
                    let fade_time_ms = 1.5;
                    let fade_samples = ((fade_time_ms / 1000.0) * original_sample_rate as f64).min(effective_len * 0.5);

                    if fade_samples > 0.0 {
                        let mut gain = 1.0_f32;
                        if relative_phase < fade_samples {
                            let t = (relative_phase / fade_samples).clamp(0.0, 1.0) as f32;
                            gain = t * t * (3.0 - 2.0 * t);
                        } else if dist_from_end < fade_samples {
                            let t = (dist_from_end / fade_samples).clamp(0.0, 1.0) as f32;
                            gain = t * t * (3.0 - 2.0 * t);
                        }
                        out_l *= gain;
                        out_r *= gain;
                    }
                }

                self.phase += phase_increment;

                if self.slice_mode {
                    if self.phase >= sub_slice_end {
                        let overshoot = self.phase - sub_slice_end;
                        self.stutter_index += 1;
                        let repeats = self.stutter_count.max(1);
                        if self.stutter_index >= repeats {
                            if self.play_mode == PlayMode::Loop {
                                self.stutter_index = 0;
                                self.phase = sub_slice_start + overshoot.rem_euclid(effective_len);
                            } else {
                                self.active = false;
                            }
                        } else {
                            self.phase = sub_slice_start + overshoot.rem_euclid(effective_len);
                        }
                    }
                } else if self.phase >= num_frames as f64 {
                    match self.play_mode {
                        PlayMode::Loop => {
                            self.phase = self.phase.rem_euclid(num_frames as f64);
                        }
                        PlayMode::OneShot => {
                            self.active = false;
                        }
                    }
                }

                self.apply_fade_out(out_l, out_r)
            }
            StretchMode::Granular => {
                let grain_dur_out = ((self.grain_size_ms.clamp(5.0, 500.0) / 1000.0) * self.sample_rate.max(1.0)).max(4.0) as f64;
                let overlap = self.overlap.clamp(1, 16);
                let spawn_interval = (grain_dur_out / overlap as f64) as f32;

                let sub_slice_start = self.slice_start.clamp(0.0, (num_frames - 1) as f64);
                let sub_slice_end = if self.slice_mode {
                    (sub_slice_start + self.sub_slice_len)
                        .min(num_frames as f64)
                        .max(sub_slice_start + 1.0)
                } else {
                    num_frames as f64
                };
                let effective_len = (sub_slice_end - sub_slice_start).max(1.0);
                let repeats = self.stutter_count.max(1);

                let playing_finished = if self.slice_mode {
                    self.play_mode == PlayMode::OneShot && self.stutter_index >= repeats
                } else {
                    self.play_mode == PlayMode::OneShot && self.phase >= num_frames as f64
                };

                // Spawn grains only while playhead is active
                if !playing_finished {
                    self.grain_spawn_timer += 1.0;
                    if self.grain_spawn_timer >= spawn_interval {
                        self.grain_spawn_timer = 0.0;
                        let random_val = self.next_random() * 2.0 - 1.0; // -1.0 to 1.0
                        if let Some(grain) = self.grains.iter_mut().find(|g| !g.active) {
                            grain.active = true;
                            grain.phase = 0.0;
                            grain.duration = grain_dur_out;

                            // Small start time jitter to reduce phasiness, clamped to active bounds
                            let jitter_range_ms = 3.0_f32;
                            let jitter_samples = (jitter_range_ms / 1000.0) * original_sample_rate;
                            let jitter = (random_val * jitter_samples) as f64;
                            let min_start = if self.slice_mode { sub_slice_start } else { 0.0 };
                            let max_start = if self.slice_mode {
                                (sub_slice_end - 1.0).max(min_start)
                            } else {
                                num_frames.saturating_sub(1) as f64
                            };
                            grain.source_start = (self.phase + jitter).clamp(min_start, max_start);
                        }
                    }
                }

                let mut out_l = 0.0;
                let mut out_r = 0.0;
                let wrap_read = !self.slice_mode && self.play_mode == PlayMode::Loop;
                let pitch_inc = (original_sample_rate as f64 / self.sample_rate.max(1.0) as f64)
                    * self.pitch_ratio.max(0.0) as f64;

                for grain in &mut self.grains {
                    if !grain.active {
                        continue;
                    }

                    // Window function (Hann)
                    let frac = (grain.phase / grain.duration.max(1.0)) as f32;
                    let window = 0.5 * (1.0 - (frac * 2.0 * std::f32::consts::PI).cos());
                    let read_pos = grain.source_start + grain.phase * pitch_inc;

                    let (g_l, g_r) = if self.slice_mode && (read_pos < sub_slice_start || read_pos >= sub_slice_end) {
                        (0.0, 0.0)
                    } else {
                        let (s_l, s_r) = Self::read_interpolated(data, channels, num_frames, read_pos, wrap_read);
                        (s_l * window, s_r * window)
                    };

                    out_l += g_l;
                    out_r += g_r;

                    grain.phase += 1.0;
                    if grain.phase >= grain.duration {
                        grain.active = false;
                    }
                }

                if !playing_finished {
                    let playhead_inc = (original_sample_rate as f64 / self.sample_rate.max(1.0) as f64)
                        * self.speed_ratio.max(0.0) as f64;
                    self.phase += playhead_inc;

                    if self.slice_mode {
                        if self.phase >= sub_slice_end {
                            let overshoot = self.phase - sub_slice_end;
                            self.stutter_index += 1;
                            if self.stutter_index >= repeats {
                                if self.play_mode == PlayMode::Loop {
                                    self.stutter_index = 0;
                                    self.phase = sub_slice_start + overshoot.rem_euclid(effective_len);
                                } else {
                                    self.phase = sub_slice_end;
                                }
                            } else {
                                self.phase = sub_slice_start + overshoot.rem_euclid(effective_len);
                            }
                        }
                    } else if self.phase >= num_frames as f64 {
                        match self.play_mode {
                            PlayMode::Loop => {
                                self.phase = self.phase.rem_euclid(num_frames as f64);
                            }
                            PlayMode::OneShot => {
                                self.phase = num_frames as f64;
                            }
                        }
                    }
                }

                let now_finished = if self.slice_mode {
                    self.play_mode == PlayMode::OneShot && self.stutter_index >= repeats
                } else {
                    self.play_mode == PlayMode::OneShot && self.phase >= num_frames as f64
                };
                if now_finished && !self.grains.iter().any(|g| g.active) {
                    self.active = false;
                }

                // Hann overlap-add sum equals overlap / 2 for overlap >= 2
                let overlap_norm = if overlap >= 2 { 2.0 / overlap as f32 } else { 1.0 };
                let gain = self.velocity * overlap_norm;
                self.apply_fade_out(out_l * gain, out_r * gain)
            }
        }
    }
}
