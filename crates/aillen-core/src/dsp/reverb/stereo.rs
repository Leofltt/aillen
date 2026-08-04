use crate::dsp::filter::biquad::{BiquadFilter, FilterType};
use crate::dsp::{AudioProcessor, StereoProcessor};

/// Multi-stage Feedback Delay Network (FDN) / Plate Reverb with Elektron-style 2-knob control:
/// - `size_time`: 0.0 (small room, fast decay) to 1.0 (infinite room sustain / freeze).
/// - `tone`: -1.0 (dark room, damped LP) through 0.0 (neutral) to 1.0 (bright room, high-cut HP).
pub struct StereoReverb {
    /// Knob 1: Size & Decay Time (0.0 to 1.0)
    pub size_time: f32,
    /// Knob 2: Tone & Color (-1.0 to 1.0)
    pub tone: f32,

    // Delay lines for 4 FDN delay paths (left/right cross-coupled)
    delay_line_1: Vec<f32>,
    delay_line_2: Vec<f32>,
    delay_line_3: Vec<f32>,
    delay_line_4: Vec<f32>,

    write_pos_1: usize,
    write_pos_2: usize,
    write_pos_3: usize,
    write_pos_4: usize,

    // Tone shaping filters in feedback loop
    lp_filter_l: BiquadFilter,
    lp_filter_r: BiquadFilter,
    hp_filter_l: BiquadFilter,
    hp_filter_r: BiquadFilter,
}

impl StereoReverb {
    pub fn new(sample_rate: f32) -> Self {
        // Prime delay lengths (in samples at 44.1kHz baseline, scaled by sample rate)
        let scale = sample_rate / 44100.0;
        let len1 = (1087.0 * scale).round() as usize;
        let len2 = (1297.0 * scale).round() as usize;
        let len3 = (1523.0 * scale).round() as usize;
        let len4 = (1787.0 * scale).round() as usize;

        let lp_l = BiquadFilter::new(sample_rate, 8000.0, 0.707, FilterType::LowPass);
        let lp_r = BiquadFilter::new(sample_rate, 8000.0, 0.707, FilterType::LowPass);
        let hp_l = BiquadFilter::new(sample_rate, 100.0, 0.707, FilterType::HighPass);
        let hp_r = BiquadFilter::new(sample_rate, 100.0, 0.707, FilterType::HighPass);

        Self {
            size_time: 0.5,
            tone: 0.0,

            delay_line_1: vec![0.0; len1],
            delay_line_2: vec![0.0; len2],
            delay_line_3: vec![0.0; len3],
            delay_line_4: vec![0.0; len4],

            write_pos_1: 0,
            write_pos_2: 0,
            write_pos_3: 0,
            write_pos_4: 0,

            lp_filter_l: lp_l,
            lp_filter_r: lp_r,
            hp_filter_l: hp_l,
            hp_filter_r: hp_r,
        }
    }

    /// Update filter cutoff parameters according to `tone` knob (-1.0 to 1.0).
    fn update_tone_filters(&mut self) {
        let t = self.tone.clamp(-1.0, 1.0);

        // Low-pass cutoff: -1.0 (Dark, 1200 Hz) -> 0.0 (8000 Hz) -> 1.0 (Bright, 18000 Hz)
        let lp_cutoff = if t < 0.0 {
            1200.0 + (t + 1.0) * (8000.0 - 1200.0)
        } else {
            8000.0 + t * (18000.0 - 8000.0)
        };

        // High-pass cutoff: -1.0 (20 Hz) -> 0.0 (100 Hz) -> 1.0 (Bright, 800 Hz)
        let hp_cutoff = if t < 0.0 {
            20.0 + (t + 1.0) * (100.0 - 20.0)
        } else {
            100.0 + t * (800.0 - 100.0)
        };

        self.lp_filter_l.set_cutoff(lp_cutoff);
        self.lp_filter_r.set_cutoff(lp_cutoff);
        self.hp_filter_l.set_cutoff(hp_cutoff);
        self.hp_filter_r.set_cutoff(hp_cutoff);
    }
}

impl StereoProcessor for StereoReverb {
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.update_tone_filters();

        // Calculate decay feedback gain based on size_time knob (0.0 -> 0.3 feedback, 1.0 -> 0.999 feedback infinite freeze)
        let feedback_gain = (0.3 + self.size_time.clamp(0.0, 1.0) * 0.699).clamp(0.0, 0.999);

        // Read delay lines
        let d1 = self.delay_line_1[self.write_pos_1];
        let d2 = self.delay_line_2[self.write_pos_2];
        let d3 = self.delay_line_3[self.write_pos_3];
        let d4 = self.delay_line_4[self.write_pos_4];

        // 4x4 Hadamard / Householder Feedback Diffusion Matrix
        let in_l = left * 0.5;
        let in_r = right * 0.5;

        let node1 = in_l + (d1 + d2 + d3 + d4) * 0.5 * feedback_gain;
        let node2 = in_l + (d1 - d2 + d3 - d4) * 0.5 * feedback_gain;
        let node3 = in_r + (d1 + d2 - d3 - d4) * 0.5 * feedback_gain;
        let node4 = in_r + (d1 - d2 - d3 + d4) * 0.5 * feedback_gain;

        // Apply tone filtering on feedback signals
        let filtered1 = self.hp_filter_l.process(self.lp_filter_l.process(node1));
        let filtered2 = self.hp_filter_l.process(self.lp_filter_l.process(node2));
        let filtered3 = self.hp_filter_r.process(self.lp_filter_r.process(node3));
        let filtered4 = self.hp_filter_r.process(self.lp_filter_r.process(node4));

        // Write into delay lines
        self.delay_line_1[self.write_pos_1] = filtered1;
        self.delay_line_2[self.write_pos_2] = filtered2;
        self.delay_line_3[self.write_pos_3] = filtered3;
        self.delay_line_4[self.write_pos_4] = filtered4;

        // Advance write positions
        self.write_pos_1 = (self.write_pos_1 + 1) % self.delay_line_1.len();
        self.write_pos_2 = (self.write_pos_2 + 1) % self.delay_line_2.len();
        self.write_pos_3 = (self.write_pos_3 + 1) % self.delay_line_3.len();
        self.write_pos_4 = (self.write_pos_4 + 1) % self.delay_line_4.len();

        // Cross-mix wet outputs (100% wet return)
        let wet_l = (filtered1 + filtered2) * 0.707;
        let wet_r = (filtered3 + filtered4) * 0.707;

        (wet_l, wet_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_decay_and_freeze() {
        let mut rev = StereoReverb::new(44100.0);
        rev.size_time = 0.99; // Infinite freeze mode

        // Feed an impulse frame
        let (_out_l1, _out_r1) = rev.process_stereo(1.0, 1.0);

        // Process silence up to first delay loopback (len1 = 1087 samples)
        for _ in 0..1087 {
            rev.process_stereo(0.0, 0.0);
        }

        let (out_l2, out_r2) = rev.process_stereo(0.0, 0.0);
        // Should still be decaying / echoing wet signal
        assert!(out_l2.abs() > 1e-4 || out_r2.abs() > 1e-4);
    }
}


