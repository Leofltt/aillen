/// A simple ring-buffered variable delay line with linear interpolation.
pub struct VariableDelay {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
}

impl VariableDelay {
    /// Creates a new `VariableDelay` line with specified sample rate and maximum delay buffer capacity.
    pub fn new(sample_rate: f32, max_delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay_samples],
            write_pos: 0,
            sample_rate,
        }
    }

    /// Pushes a new sample frame into the delay ring buffer.
    pub fn push(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
    }

    /// Reads an interpolated sample at a specified delay duration in seconds.
    pub fn read(&self, delay_sec: f32) -> f32 {
        let delay_samples = (delay_sec * self.sample_rate).clamp(0.0, (self.buffer.len() - 2) as f32);
        let read_pos = (self.write_pos as f32 - delay_samples + self.buffer.len() as f32) % self.buffer.len() as f32;

        let idx0 = read_pos.floor() as usize % self.buffer.len();
        let idx1 = (idx0 + 1) % self.buffer.len();
        let frac = read_pos - read_pos.floor();

        self.buffer[idx0] * (1.0 - frac) + self.buffer[idx1] * frac
    }
}
