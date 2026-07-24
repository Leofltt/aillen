use crate::dsp::panner::{Panner, PanMode};
use crate::synth::PlayableInstrument;
use crate::synth::two_op::two_op::TwoOpSynth;
use crate::synth::sampler::Sampler;
use crate::dsp::filter::DjFilter;
use crate::dsp::{AudioProcessor, FxChain, StereoProcessor, StereoDelay, WaveLoss};

/// A single channel strip hosting an instrument, volume level, stereo panner, and mute option.
pub struct Track {
    /// The instrument loaded into this channel.
    pub instrument: Box<dyn PlayableInstrument>,
    /// Individual track volume level.
    pub volume: f32,
    /// Pan position from -1.0 (Hard Left) to 1.0 (Hard Right).
    pub pan: f32,
    /// The constant-power panner implementation.
    panner: Panner,
    /// Whether this track is muted.
    pub mute: bool,
    /// The effects chain applied to the instrument output.
    pub fx_chain: FxChain,
    /// The send amount to the delay return track (0.0 to 1.0).
    pub send_delay: f32,
    /// The index of the track that sidechains this track, if any.
    pub sidechain_source: Option<usize>,
    /// Last processed output before panning/volume (used for sidechaining).
    pub prev_out: (f32, f32),
}

impl Track {
    /// Creates a new track with default volume, center panning, and unmuted state.
    pub fn new(instrument: Box<dyn PlayableInstrument>, sample_rate: f32) -> Self {
        Self {
            instrument,
            volume: 1.0,
            pan: 0.0,
            panner: Panner::new(0.0, PanMode::ConstantPowerSin),
            mute: false,
            fx_chain: FxChain::new(sample_rate),
            send_delay: 0.0,
            sidechain_source: None,
            prev_out: (0.0, 0.0),
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

    /// Processes a single stereo audio sample frame from this track, applying fx chain, volume, and pan settings.
    pub fn process(&mut self, sidechain_l: f32, sidechain_r: f32) -> (f32, f32) {
        if self.mute {
            self.prev_out = (0.0, 0.0);
            return (0.0, 0.0);
        }
        let (raw_l, raw_r) = self.instrument.process();
        let (fx_l, fx_r) = self.fx_chain.process_stereo_modulated(raw_l, raw_r, sidechain_l, sidechain_r);
        self.prev_out = (fx_l, fx_r);
        
        let (left_gain, right_gain) = self.panner.get_gains();
        (
            fx_l * left_gain * self.volume,
            fx_r * right_gain * self.volume,
        )
    }
}

/// The stereo master audio mixer containing all tracks, return tracks, and master volume controls.
pub struct Mixer {
    /// Dynamic track list.
    pub tracks: Vec<Track>,
    /// Global master volume level.
    pub master_volume: f32,
    /// Master stereo DJ filter left channel.
    pub master_filter_l: DjFilter,
    /// Master stereo DJ filter right channel.
    pub master_filter_r: DjFilter,
    /// The return track containing a stereo delay.
    pub return_delay: StereoDelay,
    /// Master wavelosser left channel.
    pub master_waveloss_l: WaveLoss,
    /// Master wavelosser right channel.
    pub master_waveloss_r: WaveLoss,
}

impl Mixer {
    /// Initializes a new Mixer with Track 0 and Track 1 set up for the current sample rate.
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut synth_track = Track::new(Box::new(TwoOpSynth::new(sample_rate, num_voices)), sample_rate);
        let mut sampler_track = Track::new(Box::new(Sampler::new(sample_rate, num_voices)), sample_rate);
        
        // Default cross-sidechaining configuration
        synth_track.sidechain_source = Some(1);
        sampler_track.sidechain_source = Some(0);

        let mut return_delay = StereoDelay::new(sample_rate);
        return_delay.mix = 1.0; // Full on wet by default

        Self {
            tracks: vec![synth_track, sampler_track],
            master_volume: 1.0,
            master_filter_l: DjFilter::new(sample_rate),
            master_filter_r: DjFilter::new(sample_rate),
            return_delay,
            master_waveloss_l: WaveLoss::new(),
            master_waveloss_r: WaveLoss::new(),
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

    /// Processes and sums a single stereo output sample frame across all tracks, applying return delay, master volume, master filter, and master waveloss.
    pub fn process(&mut self) -> (f32, f32) {
        // 1. Gather sidechain signals from source tracks based on their indices
        let mut sidechains = vec![(0.0f32, 0.0f32); self.tracks.len()];
        for i in 0..self.tracks.len() {
            if let Some(src_idx) = self.tracks[i].sidechain_source {
                if src_idx < self.tracks.len() {
                    sidechains[i] = self.tracks[src_idx].prev_out;
                }
            }
        }

        // 2. Process all tracks
        let mut out_l = 0.0;
        let mut out_r = 0.0;
        let mut send_l = 0.0;
        let mut send_r = 0.0;

        for i in 0..self.tracks.len() {
            let (sc_l, sc_r) = sidechains[i];
            let (track_l, track_r) = self.tracks[i].process(sc_l, sc_r);
            out_l += track_l;
            out_r += track_r;
            send_l += track_l * self.tracks[i].send_delay;
            send_r += track_r * self.tracks[i].send_delay;
        }

        // 3. Process return delay track (always 100% wet since return_delay.mix = 1.0)
        let (delay_l, delay_r) = self.return_delay.process_stereo(send_l, send_r);

        // Sum dry signals with return track output
        let master_in_l = (out_l + delay_l) * self.master_volume;
        let master_in_r = (out_r + delay_r) * self.master_volume;

        let filt_l = self.master_filter_l.process(master_in_l);
        let filt_r = self.master_filter_r.process(master_in_r);

        let (final_l, final_r) = (
            self.master_waveloss_l.process(filt_l),
            self.master_waveloss_r.process(filt_r),
        );
        (final_l, final_r)
    }

    /// Processes a single frame and returns both per-track outputs and the final master output.
    pub fn process_detailed(&mut self) -> (Vec<(f32, f32)>, (f32, f32)) {
        let mut sidechains = vec![(0.0f32, 0.0f32); self.tracks.len()];
        for i in 0..self.tracks.len() {
            if let Some(src_idx) = self.tracks[i].sidechain_source {
                if src_idx < self.tracks.len() {
                    sidechains[i] = self.tracks[src_idx].prev_out;
                }
            }
        }

        let mut track_outs = Vec::with_capacity(self.tracks.len());
        let mut out_l = 0.0;
        let mut out_r = 0.0;
        let mut send_l = 0.0;
        let mut send_r = 0.0;

        for i in 0..self.tracks.len() {
            let (sc_l, sc_r) = sidechains[i];
            let (track_l, track_r) = self.tracks[i].process(sc_l, sc_r);
            track_outs.push((track_l, track_r));
            out_l += track_l;
            out_r += track_r;
            send_l += track_l * self.tracks[i].send_delay;
            send_r += track_r * self.tracks[i].send_delay;
        }

        let (delay_l, delay_r) = self.return_delay.process_stereo(send_l, send_r);

        let master_in_l = (out_l + delay_l) * self.master_volume;
        let master_in_r = (out_r + delay_r) * self.master_volume;

        let filt_l = self.master_filter_l.process(master_in_l);
        let filt_r = self.master_filter_r.process(master_in_r);

        let final_l = self.master_waveloss_l.process(filt_l);
        let final_r = self.master_waveloss_r.process(filt_r);

        (track_outs, (final_l, final_r))
    }
}

