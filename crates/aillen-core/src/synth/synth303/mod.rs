/// Roland 303-like Synthesizer engine.
pub mod synth303;
/// Synth303 voice logic.
pub mod voice;

use crate::dsp::oscillator::Waveform;

/// Patch parameters specifying a preset configuration for the `Synth303`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Synth303Patch {
    /// Target waveform (Saw or Square).
    pub waveform: Waveform,
    /// Amplitude ADSR envelope parameters [A, D, S, R].
    pub amp_adsr: [f32; 4],
    /// Filter Cutoff ADSR envelope parameters [A, D, S, R].
    pub filter_adsr: [f32; 4],
    /// Pitch ADSR envelope parameters [A, D, S, R].
    pub pitch_adsr: [f32; 4],
    /// Base filter cutoff frequency in Hz.
    pub filter_cutoff: f32,
    /// Filter resonance (0.0 to 1.0).
    pub filter_resonance: f32,
    /// Amount of filter envelope modulation (Hz depth).
    pub filter_env_amount: f32,
    /// Amount of pitch envelope modulation (Hz depth).
    pub pitch_env_amount: f32,
    /// Pulse width of the oscillator (0.05 to 0.95).
    pub pulse_width: f32,
    /// PWM LFO modulation rate in Hz.
    pub pwm_rate: f32,
    /// PWM LFO modulation depth (0.0 to 1.0).
    pub pwm_depth: f32,
    /// Glide/Portamento time in seconds.
    pub glide_time: f32,
}

impl Default for Synth303Patch {
    /// Returns the default 303-like bass patch configuration.
    fn default() -> Self {
        Self {
            waveform: Waveform::Saw,
            // Fast attack, short decay, low sustain, moderate release for classic 303 plucks
            amp_adsr: [0.002, 0.3, 0.1, 0.2],
            filter_adsr: [0.002, 0.25, 0.05, 0.2],
            pitch_adsr: [0.002, 0.1, 0.0, 0.1],
            filter_cutoff: 300.0,
            filter_resonance: 0.75, // Highly resonant by default
            filter_env_amount: 3000.0, // Significant filter squelch
            pitch_env_amount: 0.0,
            pulse_width: 0.5,
            pwm_rate: 1.0,
            pwm_depth: 0.0, // PWM off by default
            glide_time: 0.1, // 100ms glide by default
        }
    }
}
