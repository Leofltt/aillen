/// A classic tape-style delay module with fractional delay times and ping-pong feedback.
/// Smooths delay time changes to produce analog-style pitch slides.
pub struct TapeDelay {
    sample_rate: f32,
    buffer_left: Vec<f32>,
    buffer_right: Vec<f32>,
    write_ptr: usize,
    
    // Smooth delay time tracking (in samples)
    current_delay_samples: f32,
    
    /// Target delay time in seconds.
    pub delay_time: f32,
    /// Feedback amount (0.0 to 1.0).
    pub feedback: f32,
    /// Enable ping-pong mode (feedback crosses channels).
    pub ping_pong: bool,
    /// Tape drive/saturation amount (0.0 to 1.0).
    pub drive: f32,
}

impl TapeDelay {
    /// Creates a new `TapeDelay` with a maximum delay time of 3 seconds.
    pub fn new(sample_rate: f32) -> Self {
        let buffer_size = (sample_rate * 3.0) as usize;
        Self {
            sample_rate,
            buffer_left: vec![0.0; buffer_size],
            buffer_right: vec![0.0; buffer_size],
            write_ptr: 0,
            current_delay_samples: sample_rate * 0.3, // default to 300ms
            delay_time: 0.3,
            feedback: 0.5,
            ping_pong: false,
            drive: 0.2,
        }
    }

    /// Process a stereo frame through the tape delay.
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let buffer_len = self.buffer_left.len();
        let buffer_len_f = buffer_len as f32;

        // Smooth the delay time change (simulates tape head motor movement)
        let target_delay_samples = (self.delay_time * self.sample_rate)
            .clamp(10.0, buffer_len_f - 10.0);
        
        // Dynamic response coefficient: slow enough to hear pitch sweeps when delay time is adjusted
        self.current_delay_samples += (target_delay_samples - self.current_delay_samples) * 0.0005;

        // Calculate fractional read pointers
        let read_ptr_f = (self.write_ptr as f32 - self.current_delay_samples + buffer_len_f) % buffer_len_f;
        
        // Read delayed signals with linear interpolation
        let delayed_l = self.read_interpolated(&self.buffer_left, read_ptr_f);
        let delayed_r = self.read_interpolated(&self.buffer_right, read_ptr_f);

        // Saturate/drive the feedback signal to emulate warm tape saturation
        let fb_l = self.saturate(delayed_l * self.feedback);
        let fb_r = self.saturate(delayed_r * self.feedback);

        // Write to buffers with feedback (apply ping-pong routing if enabled)
        if self.ping_pong {
            // Left feedback goes to right buffer, right feedback goes to left buffer
            self.buffer_left[self.write_ptr] = left + fb_r;
            self.buffer_right[self.write_ptr] = right + fb_l;
        } else {
            // Standard stereo feedback
            self.buffer_left[self.write_ptr] = left + fb_l;
            self.buffer_right[self.write_ptr] = right + fb_r;
        }

        // Advance write pointer
        self.write_ptr = (self.write_ptr + 1) % buffer_len;

        (delayed_l, delayed_r)
    }

    fn read_interpolated(&self, buffer: &[f32], read_ptr: f32) -> f32 {
        let len = buffer.len();
        let idx_lower = read_ptr.floor() as usize;
        let idx_upper = (idx_lower + 1) % len;
        let frac = read_ptr - read_ptr.floor();
        
        let val_lower = buffer[idx_lower % len];
        let val_upper = buffer[idx_upper];
        
        val_lower * (1.0 - frac) + val_upper * frac
    }

    /// Soft clipping saturation to simulate tape compression and warmth.
    fn saturate(&self, input: f32) -> f32 {
        if self.drive <= 0.0 {
            return input;
        }
        // Soft clipping using a simple polynomial approximation or tanh
        // Drive controls the amount of saturation/gain
        let gain = 1.0 + self.drive * 2.0;
        let driven = input * gain;
        
        // tanh approximation
        let out = driven.tanh();
        
        // Blend between clean and saturated based on drive
        input * (1.0 - self.drive) + out * self.drive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tape_delay_basic() {
        let mut delay = TapeDelay::new(44100.0);
        delay.delay_time = 0.01; // 10ms
        delay.feedback = 0.0;
        delay.ping_pong = false;
        
        // Initialize current_delay_samples directly for test speed
        delay.current_delay_samples = 441.0;

        // Process a pulse
        let (out_l, out_r) = delay.process_stereo(1.0, 1.0);
        assert_eq!(out_l, 0.0);
        assert_eq!(out_r, 0.0);

        // Process 441 samples to reach the delay time
        let mut reached = false;
        for _ in 0..441 {
            let (ol, _) = delay.process_stereo(0.0, 0.0);
            if ol > 0.0 {
                reached = true;
            }
        }
        assert!(reached);
    }
}
