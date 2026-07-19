use crate::dsp::panner::{Panner, PanMode};
use crate::synth::two_op::two_op::TwoOpSynth;
use crate::synth::sampler::Sampler;

pub enum Instrument {
    TwoOp(TwoOpSynth),
    Sampler(Sampler),
}

impl Instrument {
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

    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        match self {
            Instrument::TwoOp(synth) => synth.note_on(frequency, velocity),
            Instrument::Sampler(sampler) => sampler.note_on(frequency, velocity),
        }
    }

    pub fn note_off(&mut self, frequency: f32) {
        match self {
            Instrument::TwoOp(synth) => synth.note_off(frequency),
            Instrument::Sampler(sampler) => sampler.note_off(frequency),
        }
    }

    pub fn note_off_all(&mut self) {
        match self {
            Instrument::TwoOp(synth) => synth.note_off_all(),
            Instrument::Sampler(sampler) => sampler.note_off_all(),
        }
    }
}

pub struct Track {
    pub instrument: Instrument,
    pub volume: f32,
    pub pan: f32,
    panner: Panner,
    pub mute: bool,
}

impl Track {
    pub fn new(instrument: Instrument) -> Self {
        Self {
            instrument,
            volume: 1.0,
            pan: 0.0,
            panner: Panner::new(0.0, PanMode::ConstantPowerSin),
            mute: false,
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.max(0.0);
    }

    pub fn set_pan(&mut self, pan: f32) {
        self.pan = pan.clamp(-1.0, 1.0);
        self.panner.set_pan(self.pan);
    }

    pub fn set_mute(&mut self, mute: bool) {
        self.mute = mute;
    }

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

pub struct Mixer {
    pub tracks: [Track; 2],
    pub master_volume: f32,
}

impl Mixer {
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let synth_track = Track::new(Instrument::TwoOp(TwoOpSynth::new(sample_rate, num_voices)));
        let sampler_track = Track::new(Instrument::Sampler(Sampler::new(sample_rate, num_voices)));
        
        Self {
            tracks: [synth_track, sampler_track],
            master_volume: 1.0,
        }
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.max(0.0);
    }

    pub fn process(&mut self) -> (f32, f32) {
        let mut out_l = 0.0;
        let mut out_r = 0.0;

        for track in &mut self.tracks {
            let (l, r) = track.process();
            out_l += l;
            out_r += r;
        }

        (
            out_l * self.master_volume,
            out_r * self.master_volume,
        )
    }
}
