/// A high-performance stereo Look-Ahead Limiter to maximize master level and prevent digital clipping.
pub struct Limiter {
    sample_rate: f32,
    /// Gain boost (linear multiplier, e.g. 1.0 to 10.0) applied before limiting.
    pub threshold_gain: f32,
    /// Ceiling level (linear amplitude limit, e.g. 0.99 for -0.1 dBFS).
    pub ceiling: f32,
    /// Release time in seconds.
    pub release_s: f32,
    
    // Look-ahead delay buffers
    delay_buffer_l: Vec<f32>,
    delay_buffer_r: Vec<f32>,
    delay_write_ptr: usize,
    delay_len: usize,
    
    // Envelope follower states
    envelope: f32,
    smoothed_gain: f32,
}

impl Limiter {
    /// Creates a new `Limiter` with lookahead delay time specified in milliseconds (e.g. 2.0 ms).
    pub fn new(sample_rate: f32, lookahead_ms: f32) -> Self {
        let delay_len = ((lookahead_ms / 1000.0) * sample_rate).round() as usize;
        let delay_len = delay_len.max(1); // at least 1 sample lookahead
        
        Self {
            sample_rate,
            threshold_gain: 1.0, // no boost by default
            ceiling: 0.99,       // -0.1 dBFS
            release_s: 0.05,     // 50ms default release
            delay_buffer_l: vec![0.0; delay_len],
            delay_buffer_r: vec![0.0; delay_len],
            delay_write_ptr: 0,
            delay_len,
            envelope: 0.0,
            smoothed_gain: 1.0,
        }
    }
    
    /// Processes a stereo frame, applying pre-limit gain boost and look-ahead gain reduction.
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // 1. Apply pre-limit threshold gain boost
        let in_l = left * self.threshold_gain;
        let in_r = right * self.threshold_gain;
        
        // 2. Write to look-ahead delay buffers and retrieve delayed signals
        let read_ptr = (self.delay_write_ptr + 1) % self.delay_len;
        let delayed_l = self.delay_buffer_l[read_ptr];
        let delayed_r = self.delay_buffer_r[read_ptr];
        
        self.delay_buffer_l[self.delay_write_ptr] = in_l;
        self.delay_buffer_r[self.delay_write_ptr] = in_r;
        
        self.delay_write_ptr = read_ptr;
        
        // 3. Find absolute peak of the non-delayed input frame
        let input_peak = in_l.abs().max(in_r.abs());
        
        // 4. Envelope follower (exponential decay, instant attack)
        let release_coef = (-1.0 / (self.release_s * self.sample_rate)).exp();
        
        if input_peak > self.envelope {
            self.envelope = input_peak;
        } else {
            self.envelope = input_peak + (self.envelope - input_peak) * release_coef;
        }
        
        // 5. Calculate target gain reduction based on ceiling
        let target_gain = if self.envelope > self.ceiling {
            self.ceiling / self.envelope
        } else {
            1.0
        };
        
        // 6. Smooth the gain reduction coefficient
        if target_gain < self.smoothed_gain {
            self.smoothed_gain = target_gain; // attack is instant
        } else {
            self.smoothed_gain += (target_gain - self.smoothed_gain) * (1.0 - release_coef);
        }
        
        // Prevent denormals in follower/smoother state
        if self.envelope < 1e-15 {
            self.envelope = 0.0;
        }
        if (1.0 - self.smoothed_gain).abs() < 1e-15 {
            self.smoothed_gain = 1.0;
        }
        
        // 7. Apply gain reduction to delayed signals
        (delayed_l * self.smoothed_gain, delayed_r * self.smoothed_gain)
    }
}
