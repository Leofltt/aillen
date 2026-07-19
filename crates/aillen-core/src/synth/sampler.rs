use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::audio::{AudioBufferRef, Signal};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayMode {
    OneShot,
    Loop,
}

pub struct SampleBuffer {
    pub data: Vec<f32>,
    pub channels: usize,
    pub sample_rate: f32,
}

pub fn load_audio_file<P: AsRef<Path>>(path: P) -> Result<SampleBuffer, anyhow::Error> {
    let file = File::open(path.as_ref())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.as_ref().extension().and_then(|os| os.to_str()) {
        hint.with_extension(ext);
    }
    
    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("No supported audio track found"))?;
        
    let mut decoder = symphonia::default::get_codecs().make(
        &track.codec_params,
        &DecoderOptions::default(),
    )?;
    
    let track_id = track.id;
    
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100) as f32;
    let channels = track.codec_params.channels.map(|c: symphonia::core::audio::Channels| c.count()).unwrap_or(1);

    
    let mut data = Vec::new();
    
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        
        let decoded = decoder.decode(&packet)?;
        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        
        match decoded {
            AudioBufferRef::F32(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        data.push(buf.chan(chan)[frame]);
                    }
                }
            }
            AudioBufferRef::U8(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        data.push(buf.chan(chan)[frame] as f32 / 128.0 - 1.0);
                    }
                }
            }
            AudioBufferRef::U16(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        data.push(buf.chan(chan)[frame] as f32 / 32768.0 - 1.0);
                    }
                }
            }
            AudioBufferRef::U24(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        let sample: u32 = buf.chan(chan)[frame].0;
                        data.push(sample as f32 / 8388608.0 - 1.0);
                    }
                }
            }
            AudioBufferRef::U32(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        data.push(buf.chan(chan)[frame] as f32 / 2147483648.0 - 1.0);
                    }
                }
            }
            AudioBufferRef::S8(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        data.push(buf.chan(chan)[frame] as f32 / 128.0);
                    }
                }
            }
            AudioBufferRef::S16(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        data.push(buf.chan(chan)[frame] as f32 / 32768.0);
                    }
                }
            }
            AudioBufferRef::S24(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        let sample: i32 = buf.chan(chan)[frame].0;
                        data.push(sample as f32 / 8388608.0);
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                for frame in 0..num_frames {
                    for chan in 0..spec.channels.count() {
                        data.push(buf.chan(chan)[frame] as f32 / 2147483648.0);
                    }
                }
            }
            _ => {}
        }
    }
    
    Ok(SampleBuffer {
        data,
        channels,
        sample_rate,
    })
}

pub struct SamplerVoice {
    pub sample_rate: f32,
    pub sample_buffer: Option<Arc<SampleBuffer>>,
    pub active: bool,
    pub play_mode: PlayMode,
    pub pitch_ratio: f32,
    pub speed_ratio: f32,
    pub velocity: f32,
    pub triggered_freq: f32,
    phase: f64,
}

impl SamplerVoice {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            sample_buffer: None,
            active: false,
            play_mode: PlayMode::OneShot,
            pitch_ratio: 1.0,
            speed_ratio: 1.0,
            velocity: 0.0,
            triggered_freq: 0.0,
            phase: 0.0,
        }
    }

    pub fn set_sample(&mut self, buffer: Arc<SampleBuffer>) {
        self.sample_buffer = Some(buffer);
    }

    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        self.triggered_freq = frequency;
        self.velocity = velocity;
        self.phase = 0.0;
        self.active = self.sample_buffer.is_some();
    }

    pub fn note_off(&mut self) {
        if self.play_mode == PlayMode::Loop {
            self.active = false;
        }
    }

    pub fn process(&mut self) -> (f32, f32) {
        let buffer = match &self.sample_buffer {
            Some(b) if self.active => b,
            _ => return (0.0, 0.0),
        };

        let data = &buffer.data;
        let channels = buffer.channels;
        let original_sample_rate = buffer.sample_rate;
        let num_frames = data.len() / channels;

        if num_frames == 0 {
            self.active = false;
            return (0.0, 0.0);
        }

        // Calculate phase increment
        // Note: For now pitch_ratio and speed_ratio are multiplied, performing standard resampling.
        // If a time-stretching algorithm is introduced, it will decouple these.
        let rate_multiplier = self.pitch_ratio * self.speed_ratio;
        let phase_increment = (original_sample_rate as f64 / self.sample_rate as f64) * rate_multiplier as f64;

        let index = self.phase;
        let index_floor = index.floor() as usize;
        let index_next = index_floor + 1;
        let frac = (index - index_floor as f64) as f32;

        let mut out_l = 0.0;
        let mut out_r = 0.0;

        if channels == 1 {
            // Mono source: linear interpolation
            let s0 = data[index_floor % num_frames];
            let s1 = if index_next < num_frames {
                data[index_next]
            } else if self.play_mode == PlayMode::Loop {
                data[index_next % num_frames]
            } else {
                0.0
            };
            let sample = s0 + (s1 - s0) * frac;
            out_l = sample * self.velocity;
            out_r = sample * self.velocity;
        } else if channels >= 2 {
            // Stereo source: linear interpolation per channel
            let base0 = index_floor * channels;
            let s0_l = data[base0];
            let s0_r = data[base0 + 1];

            let (s1_l, s1_r) = if index_next < num_frames {
                let base1 = index_next * channels;
                (data[base1], data[base1 + 1])
            } else if self.play_mode == PlayMode::Loop {
                let base1 = (index_next % num_frames) * channels;
                (data[base1], data[base1 + 1])
            } else {
                (0.0, 0.0)
            };

            out_l = (s0_l + (s1_l - s0_l) * frac) * self.velocity;
            out_r = (s0_r + (s1_r - s0_r) * frac) * self.velocity;
        }

        self.phase += phase_increment;

        // Handle loop / end-of-sample
        if self.phase >= num_frames as f64 {
            match self.play_mode {
                PlayMode::Loop => {
                    self.phase -= num_frames as f64;
                }
                PlayMode::OneShot => {
                    self.active = false;
                }
            }
        }

        (out_l, out_r)
    }
}

pub struct Sampler {
    pub voices: Vec<SamplerVoice>,
    pub sample_buffer: Option<Arc<SampleBuffer>>,
    pub play_mode: PlayMode,
    pub pitch_ratio: f32,
    pub speed_ratio: f32,
    pub root_freq: f32,
}

impl Sampler {
    pub fn new(sample_rate: f32, num_voices: usize) -> Self {
        let mut voices = Vec::with_capacity(num_voices);
        for _ in 0..num_voices {
            voices.push(SamplerVoice::new(sample_rate));
        }
        Self {
            voices,
            sample_buffer: None,
            play_mode: PlayMode::OneShot,
            pitch_ratio: 1.0,
            speed_ratio: 1.0,
            root_freq: 261.63, // C4 root frequency
        }
    }

    pub fn set_sample(&mut self, buffer: SampleBuffer) {
        let arc_buf = Arc::new(buffer);
        self.sample_buffer = Some(arc_buf.clone());
        for voice in &mut self.voices {
            voice.set_sample(arc_buf.clone());
        }
    }

    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
        for voice in &mut self.voices {
            voice.play_mode = mode;
        }
    }

    pub fn set_pitch_ratio(&mut self, ratio: f32) {
        self.pitch_ratio = ratio.max(0.001);
        for voice in &mut self.voices {
            // Combine with note specific frequency differences
            voice.pitch_ratio = self.pitch_ratio * (voice.triggered_freq / self.root_freq);
        }
    }

    pub fn set_speed_ratio(&mut self, ratio: f32) {
        self.speed_ratio = ratio.max(0.001);
        for voice in &mut self.voices {
            voice.speed_ratio = self.speed_ratio;
        }
    }

    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.active) {
            voice.play_mode = self.play_mode;
            voice.speed_ratio = self.speed_ratio;
            voice.pitch_ratio = self.pitch_ratio * (frequency / self.root_freq);
            voice.note_on(frequency, velocity);
        } else {
            // Voice stealing
            let voice = &mut self.voices[0];
            voice.play_mode = self.play_mode;
            voice.speed_ratio = self.speed_ratio;
            voice.pitch_ratio = self.pitch_ratio * (frequency / self.root_freq);
            voice.note_on(frequency, velocity);
        }
    }

    pub fn note_off(&mut self, frequency: f32) {
        for voice in &mut self.voices {
            if (voice.triggered_freq - frequency).abs() < 0.01 && voice.active {
                voice.note_off();
            }
        }
    }

    pub fn note_off_all(&mut self) {
        for voice in &mut self.voices {
            voice.active = false;
        }
    }

    pub fn process(&mut self) -> (f32, f32) {
        let mut mix_l = 0.0;
        let mut mix_r = 0.0;
        let mut active_count = 0;
        
        for voice in &mut self.voices {
            if voice.active {
                let (l, r) = voice.process();
                mix_l += l;
                mix_r += r;
                active_count += 1;
            }
        }

        let headroom = 1.0 / (active_count as f32).max(1.0).sqrt();
        (mix_l * headroom, mix_r * headroom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_voice_oneshot() {
        let mut voice = SamplerVoice::new(44100.0);
        let buffer = SampleBuffer {
            data: vec![0.5, -0.5, 0.25, -0.25],
            channels: 1,
            sample_rate: 44100.0,
        };
        voice.set_sample(Arc::new(buffer));
        voice.note_on(261.63, 1.0);
        
        assert!(voice.active);

        let (l1, r1) = voice.process();
        assert_eq!(l1, 0.5);
        assert_eq!(r1, 0.5);

        let (l2, r2) = voice.process();
        assert_eq!(l2, -0.5);
        assert_eq!(r2, -0.5);
    }
}

