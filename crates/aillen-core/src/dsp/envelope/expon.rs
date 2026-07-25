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

    pub fn trigger(&mut self, start: f32, end: f32, duration: f32) {
        self.start = start.max(20.0);
        self.end = end.max(20.0);
        self.duration = duration.max(0.001);
        self.current = self.start;
        let total_samples = self.duration * self.sample_rate;
        self.factor = (self.end / self.start).powf(1.0 / total_samples);
        self.active = true;
    }

    pub fn process(&mut self) -> f32 {
        if !self.active {
            return self.end;
        }
        self.current *= self.factor;
        // Check if decay has reached target
        if (self.factor < 1.0 && self.current <= self.end) || (self.factor > 1.0 && self.current >= self.end) {
            self.current = self.end;
            self.active = false;
        }
        self.current
    }
}
