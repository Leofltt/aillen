use crate::dsp::panner::{Panner, PanMode};
use crate::synth::two_op::two_op::TwoOpSynth;
use crate::synth::sampler::Sampler;
use crate::dsp::filter::DjFilter;
use crate::dsp::AudioProcessor;

/// Represents the set of sound generation instruments supported by Aillen.
pub enum Instrument {
    /// Two-Operator FM/AM/RM/Additive Synthesizer.
    TwoOp(TwoOpSynth),
    /// Polyphonic multi-format sample playback engine.
    Sampler(Sampler),
}

impl Instrument {
    /// Processes a single sample frame from the active instrument, returning a stereo pair.
    pub fn process(&mut self) -> (f32, f32) {
        match self {
            Instrument::TwoOp(synth) => {
                use crate::dsp::AudioNode;
                let val = synth.process();
                (val, val)
            }
            Instrument::Sampler(sampler) => {
                sampler.process()
            }
        }
    }

    /// Triggers a note on the instrument at the specified frequency and velocity.
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        match self {
            Instrument::TwoOp(synth) => synth.note_on(frequency, velocity),
            Instrument::Sampler(sampler) => sampler.note_on(frequency, velocity),
        }
    }

    /// Releases a note on the instrument at the specified frequency.
    pub fn note_off(&mut self, frequency: f32) {
        match self {
            Instrument::TwoOp(synth) => synth.note_off(frequency),
            Instrument::Sampler(sampler) => sampler.note_off(frequency),
        }
    }

    /// Silences all active voices on the instrument immediately.
    pub fn note_off_all(&mut self) {
        match self {
            Instrument::TwoOp(synth) => synth.note_off_all(),
            Instrument::Sampler(sampler) => sampler.note_off_all(),
        }
    }
}

/// A single channel strip hosting an instrument, volume level, stereo panner, and mute option.
pub struct Track {
    /// The instrument loaded into this channel.
    pub instrument: Instrument,
    /// Individual track volume level.
    pub volume: f32,
    /// Pan position from -1.0 (Hard Left) to 1.0 (Hard Right).
    pub pan: f32,
    /// The constant-power panner implementation.
    panner: Panner,
    /// Whether this track is muted.
    pub mute: bool,
}

impl Track {
    /// Creates a new track with default volume, center panning, and unmuted state.
    pub fn new(instrument: Instrument) -> Self {
        Self {
            instrument,
            volume: 1.0,
            pan: 0.0,
            panner: Panner::new(0.0, PanMode::ConstantPowerSin),
            mute: false,
        }
    }

    /// Sets the volume level of this track (clamped to a minimum of 0.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.max(0.0);
    }

    /// Sets the track panning position (clamped to [-1.0, 1.0]).
    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
        self.panner.set_pan(self.pan);
    }

    /// Mutes or unmutes this track.
    pub fn set_mute(&mut self, mute: bool) {
        self.mute = mute;
    }

    /// Processes a single stereo audio sample frame from this track, applying volume and pan settings.
    pub fn process(&mut self) -> (f32, f32) {
        if self.mute {
            return (0.0, 0.0);
        }
        let (raw_l, raw_r) = self.instrument.process();
        
        let (left_gain, right_gain) = self.panner.get_gains();
        (
            raw_l * left_gain * self.volume,
            raw_r * right_gain * self.volume,
        )
    }
}

/// The stereo master audio mixer containing all tracks and master volume controls.
pub struct Mixer {
    /// Fixed-size track list: Track 0 (TwoOp Synth) and Track 1 (Sampler).
    pub tracks: [Track; 2],
    /// Global master volume level.
    pub master_volume: f32,
    /// Master stereo DJ filter left channel.
    pub master_filter_l: DjFilter,
    /// Master stereo DJ filter right channel.
    pub master_filter_r: DjFilter,
}

impl Mixer {
    /// Initializes a new Mixer with Track 0 and Track 1 set up for the current sample rate.
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let synth_track = Track::new(Instrument::TwoOp(TwoOpSynth::new(sample_rate, num_voices)));
        let sampler_track = Track::new(Instrument::Sampler(Sampler::new(sample_rate, num_voices)));
        
        Self {
            tracks: [synth_track, sampler_track],
            master_volume: 1.0,
            master_filter_l: DjFilter::new(sample_rate),
            master_filter_r: DjFilter::new(sample_rate),
        }
    }

    /// Sets the master volume level (clamped to a minimum of 0.0).
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.max(0.0);
    }

    /// Sets the master DJ filter position.
    pub fn set_master_filter_position(&mut self, pos: f32) {
        self.master_filter_l.set_position(pos);
        self.master_filter_r.set_position(pos);
    }

    /// Processes and sums a single stereo output sample frame across all tracks, applying master volume and master filter.
    pub fn process(&mut self) -> (f32, f32) {
        let mut out_l = 0.0;
        let mut out_r = 0.0;

        for track in &mut self.tracks {
            let (l, r) = track.process();
            out_l += l;
            out_r += r;
        }

        let summed_l = out_l * self.master_volume;
        let summed_r = out_r * self.master_volume;

        (
            self.master_filter_l.process(summed_l),
            self.master_filter_r.process(summed_r),
        )
    }
}
