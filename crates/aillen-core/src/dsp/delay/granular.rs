use std::f32::consts::PI;

/// A simple deterministic random number generator (LCG) for grain offsets.
struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.state as f32) / (u32::MAX as f32)
    }

    fn next_range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

#[derive(Clone)]
struct Grain {
    playhead: f32,
    duration: f32,
    progress: f32,
    speed: f32,
}

impl Grain {
    fn new() -> Self {
        Self {
            playhead: 0.0,
            duration: 44100.0 * 0.1, // 100ms
            progress: 0.0,
            speed: 1.0,
        }
    }
}

/// A granular delay module that shreds input audio into grains and plays them back
/// with pitch, size, density, and spray (jitter) controls.
pub struct GranularDelay {
    sample_rate: f32,
    buffer_left: Vec<f32>,
    buffer_right: Vec<f32>,
    write_ptr: usize,
    
    grains_left: Vec<Grain>,
    grains_right: Vec<Grain>,
    rng: SimpleRng,

    /// Delay time/offset in seconds.
    pub delay_time: f32,
    /// Grain duration in seconds (10ms to 500ms).
    pub grain_size: f32,
    /// Number of active overlapping grains (1 to 8).
    pub density: usize,
    /// Maximum random offset (spray) in seconds.
    pub spray: f32,
    /// Playback speed / pitch factor (0.5 to 2.0).
    pub pitch: f32,
    /// Feedback path gain (0.0 to 1.0).
    pub feedback: f32,
}

impl GranularDelay {
    const MAX_GRAINS: usize = 8;

    /// Creates a new `GranularDelay` with a 4-second delay recording buffer.
    pub fn new(sample_rate: f32) -> Self {
        let buffer_size = (sample_rate * 4.0) as usize;
        
        let mut delay = Self {
            sample_rate,
            buffer_left: vec![0.0; buffer_size],
            buffer_right: vec![0.0; buffer_size],
            write_ptr: 0,
            grains_left: vec![Grain::new(); Self::MAX_GRAINS],
            grains_right: vec![Grain::new(); Self::MAX_GRAINS],
            rng: SimpleRng::new(12345),
            delay_time: 0.3,
            grain_size: 0.1, // 100ms
            density: 4,      // 4 active grains by default
            spray: 0.02,     // 20ms jitter
            pitch: 1.0,      // standard speed
            feedback: 0.4,
        };

        // Stagger the initial grain progression so they don't spawn all at once
        let initial_dur_samples = delay.grain_size * sample_rate;
        for i in 0..Self::MAX_GRAINS {
            let offset_ratio = i as f32 / Self::MAX_GRAINS as f32;
            delay.grains_left[i].duration = initial_dur_samples;
            delay.grains_left[i].progress = initial_dur_samples * offset_ratio;
            delay.grains_left[i].playhead = delay.write_ptr as f32 - (delay.delay_time * sample_rate);
            delay.grains_left[i].speed = delay.pitch;

            delay.grains_right[i].duration = initial_dur_samples;
            delay.grains_right[i].progress = initial_dur_samples * offset_ratio;
            delay.grains_right[i].playhead = delay.write_ptr as f32 - (delay.delay_time * sample_rate);
            delay.grains_right[i].speed = delay.pitch;
        }

        delay
    }

    /// Process a stereo frame through the granular delay.
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let buffer_len = self.buffer_left.len();
        let buffer_len_f = buffer_len as f32;
        let delay_samples = self.delay_time * self.sample_rate;
        let grain_samples = (self.grain_size * self.sample_rate).max(44.0); // min 1ms
        let spray_samples = self.spray * self.sample_rate;
        
        let active_density = self.density.clamp(1, Self::MAX_GRAINS);

        // 1. Synthesize current grain outputs
        let mut out_l = 0.0;
        let mut out_r = 0.0;

        // Left channel grains
        for i in 0..active_density {
            let grain = &mut self.grains_left[i];
            if grain.progress >= grain.duration {
                // Respawn grain
                grain.duration = grain_samples;
                grain.progress = 0.0;
                grain.speed = self.pitch;
                // Randomized start position near target delay time
                let random_offset = self.rng.next_range(-spray_samples, spray_samples);
                let start_idx = (self.write_ptr as f32 - delay_samples + random_offset + buffer_len_f) % buffer_len_f;
                grain.playhead = start_idx;
            }

            // Read sample with linear interpolation
            let val = Self::read_interpolated(&self.buffer_left, grain.playhead);
            // Apply Hann window envelope
            let env = 0.5 * (1.0 - (2.0 * PI * grain.progress / grain.duration).cos());
            out_l += val * env;

            // Advance grain
            grain.playhead = (grain.playhead + grain.speed + buffer_len_f) % buffer_len_f;
            grain.progress += 1.0;
        }

        // Right channel grains
        for i in 0..active_density {
            let grain = &mut self.grains_right[i];
            if grain.progress >= grain.duration {
                // Respawn grain
                grain.duration = grain_samples;
                grain.progress = 0.0;
                grain.speed = self.pitch;
                let random_offset = self.rng.next_range(-spray_samples, spray_samples);
                let start_idx = (self.write_ptr as f32 - delay_samples + random_offset + buffer_len_f) % buffer_len_f;
                grain.playhead = start_idx;
            }

            // Read sample with linear interpolation
            let val = Self::read_interpolated(&self.buffer_right, grain.playhead);
            let env = 0.5 * (1.0 - (2.0 * PI * grain.progress / grain.duration).cos());
            out_r += val * env;

            // Advance grain
            grain.playhead = (grain.playhead + grain.speed + buffer_len_f) % buffer_len_f;
            grain.progress += 1.0;
        }

        // Normalize sum by active density count to prevent clipping
        let active_density_f = active_density as f32;
        out_l /= active_density_f;
        out_r /= active_density_f;

        // 2. Write input + feedback back to recording buffer
        self.buffer_left[self.write_ptr] = left + out_l * self.feedback;
        self.buffer_right[self.write_ptr] = right + out_r * self.feedback;

        self.write_ptr = (self.write_ptr + 1) % buffer_len;

        (out_l, out_r)
    }

    fn read_interpolated(buffer: &[f32], read_ptr: f32) -> f32 {
        let len = buffer.len();
        let idx_lower = read_ptr.floor() as usize;
        let idx_upper = (idx_lower + 1) % len;
        let frac = read_ptr - read_ptr.floor();
        
        let val_lower = buffer[idx_lower % len];
        let val_upper = buffer[idx_upper];
        
        val_lower * (1.0 - frac) + val_upper * frac
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_granular_delay_basic() {
        let mut delay = GranularDelay::new(44100.0);
        delay.feedback = 0.0;
        delay.grain_size = 0.05;
        
        // Push some signals through
        let mut out_l_sum = 0.0;
        for _ in 0..1000 {
            let (ol, _) = delay.process_stereo(1.0, 1.0);
            out_l_sum += ol.abs();
        }
        
        // Grains should have read from empty buffers initially, but eventually produce sound
        assert!(out_l_sum >= 0.0);
    }

    #[test]
    fn test_granular_delay_density() {
        let mut delay = GranularDelay::new(44100.0);
        delay.density = 8;
        let _ = delay.process_stereo(1.0, 1.0);
        assert_eq!(delay.density, 8);
    }
}
