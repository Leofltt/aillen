/// A pitch/cutoff exponential decay envelope mimicking the Csound `expon` behavior.
pub struct ExponEnvelope {
    sample_rate: f32,
    pub start: f32,
    pub end: f32,
    pub duration: f32,
    current: f32,
    factor: f32,
    active: bool,
}

impl ExponEnvelope {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            start: 1000.0,
            end: 200.0,
            duration: 1.0,
            current: 1000.0,
            factor: 0.999,
            active: false,
        }
    }

    /// Triggers an exponential one-shot sweep from `start` to `end` over `duration` seconds.
    pub fn trigger(&mut self, start: f32, end: f32, duration: f32) {
        // Enforce positive values for ratio computation to avoid NaN
        self.start = if start.is_nan() || start <= 1e-4 { 1e-4 } else { start };
        self.end = if end.is_nan() || end <= 1e-4 { 1e-4 } else { end };
        self.duration = duration.max(0.0001);
        self.current = self.start;

        let total_samples = (self.duration * self.sample_rate).max(1.0);
        let ratio = self.end / self.start;
        self.factor = ratio.powf(1.0 / total_samples);
        self.active = true;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn process(&mut self) -> f32 {
        if !self.active {
            return self.end;
        }
        self.current *= self.factor;
        // Check if decay has reached or passed target (with small epsilon for float precision)
        if (self.factor <= 1.0 && self.current <= self.end + 1e-5) || (self.factor >= 1.0 && self.current >= self.end - 1e-5) {
            self.current = self.end;
            self.active = false;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expon_envelope_pitch_drop() {
        let mut env = ExponEnvelope::new(100.0);
        env.trigger(4.0, 1.0, 0.1); // 10 samples from 4.0 to 1.0
        assert_eq!(env.current, 4.0);
        for _ in 0..10 {
            env.process();
        }
        assert_eq!(env.current, 1.0);
        assert!(!env.is_active());
    }

    #[test]
    fn test_expon_envelope_safe_zero_handling() {
        let mut env = ExponEnvelope::new(100.0);
        env.trigger(0.0, 0.0, 0.1);
        let val = env.process();
        assert!(!val.is_nan());
    }
}
