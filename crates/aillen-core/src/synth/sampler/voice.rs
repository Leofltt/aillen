use std::sync::Arc;
use super::buffer::{SampleBuffer, PlayMode, StretchMode};

/// Represents an individual active sound grain in the granular engine.
#[derive(Clone, Copy, Debug, Default)]
pub struct Grain {
    /// The starting index in the source buffer.
    pub source_start: f64,
    /// The current phase offset within the grain.
    pub phase: f64,
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
}

impl SamplerVoice {
    /// Creates a new SamplerVoice configured for the target sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            sample_buffer: None,
            active: false,
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
        }
    }

    /// Returns a pseudo-random float between 0.0 and 1.0 using a fast LCG.
    fn next_random(&mut self) -> f32 {
        self.rng_seed = self.rng_seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.rng_seed as f32) / (u32::MAX as f32)
    }

    /// Finds the closest zero-crossing point in a window around target_frame.
    fn find_closest_zero_crossing(&self, data: &[f32], target_frame: usize, channels: usize, window: usize) -> usize {
        let num_frames = data.len() / channels;
        if num_frames == 0 {
            return 0;
        }
        
        let mut best_frame = target_frame;
        let mut min_abs = f32::MAX;
        
        let start_search = target_frame.saturating_sub(window);
        let end_search = (target_frame + window).min(num_frames);
        
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
        
        for grain in &mut self.grains {
            grain.active = false;
        }
        
        if let Some(ref buffer) = self.sample_buffer {
            let original_sample_rate = buffer.sample_rate;
            let num_frames = buffer.data.len() / buffer.channels;
            
            if self.slice_mode && num_frames > 0 {
                let slice_len = num_frames as f64 / self.num_slices.max(1) as f64;
                let target_start = (self.selected_slice.min(self.num_slices - 1) as f64 * slice_len) as usize;
                let adjusted_start = self.find_closest_zero_crossing(&buffer.data, target_start, buffer.channels, 1000);
                self.slice_start = adjusted_start as f64;
                self.sub_slice_len = slice_len / self.stutter_count.max(1) as f64;
                self.phase = self.slice_start;
            } else {
                self.slice_start = 0.0;
                self.sub_slice_len = num_frames as f64;
            }

            let grain_size_samples = (self.grain_size_ms / 1000.0) * original_sample_rate;
            let spawn_interval = grain_size_samples / self.overlap.max(1) as f32;
            self.grain_spawn_timer = spawn_interval;
        }

        self.active = self.sample_buffer.is_some();
    }

    /// Releases the active note.
    pub fn note_off(&mut self) {
        if self.play_mode == PlayMode::Loop {
            self.active = false;
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
        let num_frames = data.len() / channels;

        if num_frames == 0 {
            self.active = false;
            return (0.0, 0.0);
        }

        match self.stretch_mode {
            StretchMode::Resample => {
                let rate_multiplier = self.pitch_ratio * self.speed_ratio;
                let phase_increment = (original_sample_rate as f64 / self.sample_rate as f64) * rate_multiplier as f64;

                let index = self.phase;
                let index_floor = index.floor() as usize;
                let index_next = index_floor + 1;
                let frac = (index - index_floor as f64) as f32;

                let mut out_l = 0.0;
                let mut out_r = 0.0;

                if channels == 1 {
                    let s0 = data[index_floor % num_frames];
                    let s1 = if index_next < num_frames {
                        data[index_next]
                    } else if self.play_mode == PlayMode::Loop {
                        data[index_next % num_frames]
                    } else {
                        0.0
                    };
                    let sample = s0 + (s1 - s0) * frac;
                    out_l = sample * self.velocity;
                    out_r = sample * self.velocity;
                } else if channels >= 2 {
                    let base0 = (index_floor % num_frames) * channels;
                    let s0_l = data[base0];
                    let s0_r = data[base0 + 1];

                    let (s1_l, s1_r) = if index_next < num_frames {
                        let base1 = index_next * channels;
                        (data[base1], data[base1 + 1])
                    } else if self.play_mode == PlayMode::Loop {
                        let base1 = (index_next % num_frames) * channels;
                        (data[base1], data[base1 + 1])
                    } else {
                        (0.0, 0.0)
                    };

                    out_l = (s0_l + (s1_l - s0_l) * frac) * self.velocity;
                    out_r = (s0_r + (s1_r - s0_r) * frac) * self.velocity;
                }

                self.phase += phase_increment;

                if self.slice_mode {
                    let sub_slice_end = self.slice_start + (self.stutter_index + 1) as f64 * self.sub_slice_len;
                    if self.phase >= sub_slice_end {
                        self.stutter_index += 1;
                        if self.stutter_index >= self.stutter_count {
                            self.active = false;
                        } else {
                            self.phase = self.slice_start;
                        }
                    }
                } else {
                    if self.phase >= num_frames as f64 {
                        match self.play_mode {
                            PlayMode::Loop => {
                                self.phase -= num_frames as f64;
                            }
                            PlayMode::OneShot => {
                                self.active = false;
                            }
                        }
                    }
                }

                (out_l, out_r)
            }
            StretchMode::Granular => {
                let grain_size_samples = (self.grain_size_ms / 1000.0) * original_sample_rate;
                let spawn_interval = grain_size_samples / self.overlap.max(1) as f32;

                // Spawn logic
                self.grain_spawn_timer += 1.0;
                if self.grain_spawn_timer >= spawn_interval {
                    self.grain_spawn_timer = 0.0;
                    
                    let can_spawn = match self.play_mode {
                        PlayMode::Loop => !self.slice_mode || self.stutter_index < self.stutter_count,
                        PlayMode::OneShot => self.phase < num_frames as f64 && (!self.slice_mode || self.stutter_index < self.stutter_count),
                    };

                    if can_spawn {
                        let random_val = self.next_random() * 2.0 - 1.0; // -1.0 to 1.0
                        if let Some(grain) = self.grains.iter_mut().find(|g| !g.active) {
                            grain.active = true;
                            grain.phase = 0.0;
                            
                            // Add a small start time jitter to reduce phasiness / periodic interference
                            let jitter_range_ms = 3.0; // +/- 3ms of jitter
                            let jitter_samples = (jitter_range_ms / 1000.0) * original_sample_rate;
                            let jitter = (random_val * jitter_samples) as f64;
                            
                            grain.source_start = (self.phase + jitter).clamp(0.0, num_frames.saturating_sub(1) as f64);
                        }
                    }
                }

                let mut out_l = 0.0;
                let mut out_r = 0.0;
                let mut active_grains_count = 0;

                for grain in &mut self.grains {
                    if !grain.active {
                        continue;
                    }
                    active_grains_count += 1;

                    let read_pos = grain.source_start + grain.phase;

                    // Window function (Hanning)
                    let frac = (grain.phase / grain_size_samples as f64) as f32;
                    let window = 0.5 * (1.0 - (frac * 2.0 * std::f32::consts::PI).cos());

                    let index_floor = read_pos.floor() as usize;
                    let index_next = index_floor + 1;
                    let interp_frac = (read_pos - index_floor as f64) as f32;

                    let mut g_l = 0.0;
                    let mut g_r = 0.0;

                    if channels == 1 {
                        let s0 = data[index_floor % num_frames];
                        let s1 = if index_next < num_frames {
                            data[index_next]
                        } else if self.play_mode == PlayMode::Loop {
                            data[index_next % num_frames]
                        } else {
                            0.0
                        };
                        let sample = s0 + (s1 - s0) * interp_frac;
                        g_l = sample * window;
                        g_r = sample * window;
                    } else if channels >= 2 {
                        let base0 = (index_floor % num_frames) * channels;
                        let s0_l = data[base0];
                        let s0_r = data[base0 + 1];

                        let (s1_l, s1_r) = if index_next < num_frames {
                            let base1 = index_next * channels;
                            (data[base1], data[base1 + 1])
                        } else if self.play_mode == PlayMode::Loop {
                            let base1 = (index_next % num_frames) * channels;
                            (data[base1], data[base1 + 1])
                        } else {
                            (0.0, 0.0)
                        };

                        g_l = (s0_l + (s1_l - s0_l) * interp_frac) * window;
                        g_r = (s0_r + (s1_r - s0_r) * interp_frac) * window;
                    }

                    out_l += g_l;
                    out_r += g_r;

                    let pitch_inc = (original_sample_rate as f64 / self.sample_rate as f64) * self.pitch_ratio as f64;
                    grain.phase += pitch_inc;

                    if grain.phase >= grain_size_samples as f64 {
                        grain.active = false;
                    }
                }

                let playhead_inc = (original_sample_rate as f64 / self.sample_rate as f64) * self.speed_ratio as f64;
                self.phase += playhead_inc;

                if self.slice_mode {
                    let sub_slice_end = self.slice_start + (self.stutter_index + 1) as f64 * self.sub_slice_len;
                    if self.phase >= sub_slice_end {
                        self.stutter_index += 1;
                        if self.stutter_index >= self.stutter_count {
                            if active_grains_count == 0 {
                                self.active = false;
                            }
                        } else {
                            self.phase = self.slice_start;
                        }
                    }
                } else {
                    if self.phase >= num_frames as f64 {
                        match self.play_mode {
                            PlayMode::Loop => {
                                self.phase -= num_frames as f64;
                            }
                            PlayMode::OneShot => {
                                if active_grains_count == 0 {
                                    self.active = false;
                                }
                            }
                        }
                    }
                }

                (out_l * self.velocity, out_r * self.velocity)
            }
        }
    }
}
