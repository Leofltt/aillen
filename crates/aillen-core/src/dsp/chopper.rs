use std::f32::consts::PI;
use crate::dsp::StereoProcessor;

const FADE_TIME_SEC: f32 = 0.0015;

/// Unipolar window/waveform shapes supported by the rhythmic Chopper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChopperShape {
    Square = 0,
    Hann = 1,
    RampDown = 2,
    RampUp = 3,
    Cosine = 4,
}

impl ChopperShape {
    /// Converts an integer OSC argument to a `ChopperShape`, defaulting to `Square`.
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => ChopperShape::Hann,
            2 => ChopperShape::RampDown,
            3 => ChopperShape::RampUp,
            4 => ChopperShape::Cosine,
            _ => ChopperShape::Square,
        }
    }
}

/// Timing synchronization modes for the Chopper LFO rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChopperSyncMode {
    /// `rate` divides the triggered note duration (`dur` / `durTS`); phase resets on note trigger.
    NoteDur = 0,
    /// `rate` divides the active sample/slice playback length on Sampler tracks; phase resets on note trigger.
    SampleLength = 1,
    /// `rate` is treated directly as free-running LFO frequency in Hz (phase does not reset).
    FreeHz = 2,
}

impl ChopperSyncMode {
    /// Converts an integer OSC argument to a `ChopperSyncMode`, defaulting to `NoteDur`.
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => ChopperSyncMode::SampleLength,
            2 => ChopperSyncMode::FreeHz,
            _ => ChopperSyncMode::NoteDur,
        }
    }
}

/// Stereo-linked, unipolar LFO rhythmic amplitude chopper / gate.
pub struct Chopper {
    sample_rate: f32,
    /// Number of divisions per note/sample length (in synced modes) or frequency in Hz (`FreeHz`).
    /// `0.0` bypasses the effect.
    pub rate: f32,
    /// Window/waveform shape applied to each cycle.
    pub shape: ChopperShape,
    /// Modulation depth (`0.0` = dry/bypass, `1.0` = full gating down to silence).
    pub depth: f32,
    /// Pulse width / active window fraction per cycle (`0.05` to `1.0`).
    pub duty: f32,
    /// Timing synchronization mode.
    pub sync_mode: ChopperSyncMode,

    note_duration_ms: f32,
    sample_duration_ms: Option<f32>,
    has_explicit_note_dur: bool,
    phase: f32,
}

impl Chopper {
    /// Creates a new `Chopper` bypassed by default (`rate = 0.0`, `depth = 1.0`, `duty = 0.5`).
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1.0),
            rate: 0.0,
            shape: ChopperShape::Square,
            depth: 1.0,
            duty: 0.5,
            sync_mode: ChopperSyncMode::NoteDur,
            note_duration_ms: 1000.0,
            sample_duration_ms: None,
            has_explicit_note_dur: false,
            phase: 0.0,
        }
    }

    /// Sets the chopper division rate or free-running Hz frequency (`0.0` = bypass).
    pub fn set_rate(&mut self, rate: f32) {
        self.rate = rate.max(0.0);
    }

    /// Sets the amplitude modulation depth (`0.0` to `1.0`).
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.clamp(0.0, 1.0);
    }

    /// Sets the active window duty cycle (`0.05` to `1.0`).
    pub fn set_duty(&mut self, duty: f32) {
        self.duty = duty.clamp(0.05, 1.0);
    }

    /// Updates note/sample timing context on note trigger and resets phase in synced modes.
    pub fn trigger_note(&mut self, note_duration_ms: Option<f32>, sample_duration_ms: Option<f32>) {
        if let Some(d) = note_duration_ms {
            self.note_duration_ms = d.max(1.0);
            self.has_explicit_note_dur = true;
        }
        if let Some(s) = sample_duration_ms {
            let s_clamped = s.max(1.0);
            self.sample_duration_ms = Some(s_clamped);
            if !self.has_explicit_note_dur {
                self.note_duration_ms = s_clamped;
            }
        }
        if self.sync_mode != ChopperSyncMode::FreeHz {
            self.phase = 0.0;
        }
    }

    /// Computes the active LFO frequency in Hz based on `rate`, `sync_mode`, and timing context.
    pub fn effective_frequency(&self) -> f32 {
        if self.rate <= 1e-5 {
            return 0.0;
        }
        let raw_hz = match self.sync_mode {
            ChopperSyncMode::NoteDur => {
                let dur_sec = (self.note_duration_ms / 1000.0).max(0.001);
                self.rate / dur_sec
            }
            ChopperSyncMode::SampleLength => {
                let dur_ms = self.sample_duration_ms.unwrap_or(self.note_duration_ms);
                let dur_sec = (dur_ms / 1000.0).max(0.001);
                self.rate / dur_sec
            }
            ChopperSyncMode::FreeHz => self.rate,
        };
        let max_hz = (self.sample_rate * 0.25).max(0.001);
        raw_hz.clamp(0.001, max_hz)
    }

    fn smoothstep(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    fn evaluate_window(&self, p: f32, freq: f32) -> f32 {
        let duty = self.duty.clamp(0.05, 1.0);
        match self.shape {
            ChopperShape::Square => {
                if p >= duty {
                    return 0.0;
                }
                let u = p / duty;
                let edge_u = ((FADE_TIME_SEC * freq) / duty).min(0.25);
                if edge_u > 1e-6 && u < edge_u {
                    Self::smoothstep(u / edge_u)
                } else if edge_u > 1e-6 && u > 1.0 - edge_u {
                    Self::smoothstep((1.0 - u) / edge_u)
                } else {
                    1.0
                }
            }
            ChopperShape::Hann => {
                if p >= duty {
                    return 0.0;
                }
                let u = p / duty;
                0.5 * (1.0 - (2.0 * PI * u).cos())
            }
            ChopperShape::RampDown => {
                if p >= duty {
                    return 0.0;
                }
                let u = p / duty;
                let edge_u = ((FADE_TIME_SEC * freq) / duty).min(0.15);
                if edge_u > 1e-6 && u < edge_u {
                    Self::smoothstep(u / edge_u)
                } else {
                    ((1.0 - u) / (1.0 - edge_u).max(1e-6)).clamp(0.0, 1.0)
                }
            }
            ChopperShape::RampUp => {
                if p >= duty {
                    return 0.0;
                }
                let u = p / duty;
                let edge_u = ((FADE_TIME_SEC * freq) / duty).min(0.15);
                if edge_u > 1e-6 && u > 1.0 - edge_u {
                    Self::smoothstep((1.0 - u) / edge_u)
                } else {
                    (u / (1.0 - edge_u).max(1e-6)).clamp(0.0, 1.0)
                }
            }
            ChopperShape::Cosine => {
                let dist = p.min(1.0 - p);
                let half_w = 0.5 * duty;
                if dist < half_w {
                    0.5 * (1.0 + (PI * dist / half_w).cos())
                } else {
                    0.0
                }
            }
        }
    }
}

impl StereoProcessor for Chopper {
    /// Applies stereo-linked unipolar amplitude modulation to a stereo sample frame.
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.rate <= 1e-5 || self.depth <= 1e-5 {
            return (left, right);
        }

        let freq = self.effective_frequency();
        let window = self.evaluate_window(self.phase, freq);
        let depth = self.depth.clamp(0.0, 1.0);
        let gain = 1.0 - depth * (1.0 - window);

        self.phase += freq / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
        }

        let raw_l = left * gain;
        let raw_r = right * gain;
        let out_l = if raw_l.abs() < 1e-15 { 0.0 } else { raw_l };
        let out_r = if raw_r.abs() < 1e-15 { 0.0 } else { raw_r };
        (out_l, out_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chopper_bypass_by_default() {
        let mut chopper = Chopper::new(44100.0);
        assert_eq!(chopper.effective_frequency(), 0.0);
        let (l, r) = chopper.process_stereo(0.42, -0.73);
        assert!((l - 0.42).abs() < 1e-6);
        assert!((r + 0.73).abs() < 1e-6);
    }

    #[test]
    fn test_chopper_note_dur_sync_frequency() {
        let mut chopper = Chopper::new(44100.0);
        chopper.set_rate(4.0);
        chopper.trigger_note(Some(500.0), None);
        assert!((chopper.effective_frequency() - 8.0).abs() < 1e-5);
    }

    #[test]
    fn test_chopper_sample_length_mode_and_fallback() {
        let mut chopper = Chopper::new(44100.0);
        chopper.sync_mode = ChopperSyncMode::SampleLength;
        chopper.set_rate(2.0);
        chopper.trigger_note(Some(500.0), None);
        assert!((chopper.effective_frequency() - 4.0).abs() < 1e-5);

        chopper.trigger_note(Some(500.0), Some(250.0));
        assert!((chopper.effective_frequency() - 8.0).abs() < 1e-5);
    }

    #[test]
    fn test_chopper_free_hz_does_not_reset_phase() {
        let mut chopper = Chopper::new(44100.0);
        chopper.sync_mode = ChopperSyncMode::FreeHz;
        chopper.set_rate(10.0);
        for _ in 0..100 {
            chopper.process_stereo(1.0, 1.0);
        }
        let phase_before = chopper.phase;
        assert!(phase_before > 0.0);
        chopper.trigger_note(Some(500.0), None);
        assert!((chopper.phase - phase_before).abs() < 1e-6);
    }

    #[test]
    fn test_chopper_shapes_bounds() {
        let mut chopper = Chopper::new(44100.0);
        chopper.set_rate(4.0);
        chopper.set_duty(0.5);
        let freq = 4.0;

        for shape_idx in 0..=4 {
            chopper.shape = ChopperShape::from_i32(shape_idx);
            for step in 0..100 {
                let p = step as f32 / 100.0;
                let w = chopper.evaluate_window(p, freq);
                assert!((0.0..=1.0).contains(&w), "shape {} out of bounds at {}: {}", shape_idx, p, w);
            }
        }

        chopper.shape = ChopperShape::Square;
        assert!((chopper.evaluate_window(0.25, freq) - 1.0).abs() < 1e-5);
        assert!(chopper.evaluate_window(0.75, freq) < 1e-5);

        chopper.shape = ChopperShape::Cosine;
        assert!((chopper.evaluate_window(0.0, freq) - 1.0).abs() < 1e-5);

        chopper.shape = ChopperShape::Hann;
        assert!((chopper.evaluate_window(0.25, freq) - 1.0).abs() < 1e-5);
    }
}
