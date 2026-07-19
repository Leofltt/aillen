use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::audio::{AudioBufferRef, Signal};

/// Modes of playback for loaded audio samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayMode {
    /// Play the sample once from start to finish.
    OneShot,
    /// Loop playback from the start of the sample once the end is reached.
    Loop,
}

/// A container holding raw audio sample data along with its channel count and native sample rate.
pub struct SampleBuffer {
    /// Interleaved PCM sample values.
    pub data: Vec<f32>,
    /// Number of channels (e.g. 1 for mono, 2 for stereo).
    pub channels: usize,
    /// Original sampling rate of the file in Hz.
    pub sample_rate: f32,
}

/// Loads and decodes an audio file from a path into a `SampleBuffer`.
///
/// Supports major formats such as WAV, MP3, and FLAC using Symphonia decoders.
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

/// The mode used to pitch-shift and time-stretch loaded samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StretchMode {
    /// Traditional pitch shifting where speed and pitch are linked (like tape).
    Resample,
    /// Granular overlap-add pitch/time decoupling (retro time-stretching).
    Granular,
}

/// Represents an individual active sound grain in the granular engine.
#[derive(Clone, Copy, Debug, Default)]
pub struct Grain {
    /// The starting index in the source buffer.
    pub source_start: f64,
    /// The current phase offset within the grain.
    pub phase: f64,
    /// Whether this grain is active.
    pub active: bool,
}

/// Represents a single polyphonic voice playing back a sample buffer.
pub struct SamplerVoice {
    /// Audio stream sample rate.
    pub sample_rate: f32,
    /// Reference-counted shared sample buffer.
    pub sample_buffer: Option<Arc<SampleBuffer>>,
    /// Whether the voice is active.
    pub active: bool,
    /// Playback mode (Loop or OneShot).
    pub play_mode: PlayMode,
    /// Current pitch shifting factor.
    pub pitch_ratio: f32,
    /// Current playback speed factor.
    pub speed_ratio: f32,
    /// Gain velocity factor.
    pub velocity: f32,
    /// Triggered MIDI note frequency.
    pub triggered_freq: f32,
    /// Main playback position index.
    phase: f64,

    /// Playback stretch method (Resample or Granular).
    pub stretch_mode: StretchMode,
    /// Size of grains in milliseconds (e.g. 40.0 ms).
    pub grain_size_ms: f32,
    /// Number of active overlapping grains (e.g. 4).
    pub overlap: usize,
    /// Fixed array storing active grain data.
    grains: [Grain; 16],
    /// Timer determining when to spawn the next grain.
    grain_spawn_timer: f32,
}

impl SamplerVoice {
    /// Creates a new SamplerVoice configured for the target sample rate.
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
            stretch_mode: StretchMode::Resample,
            grain_size_ms: 40.0,
            overlap: 4,
            grains: [Grain::default(); 16],
            grain_spawn_timer: 999999.0, // Force spawn immediately on start
        }
    }

    /// Sets the shared sample buffer for this voice.
    pub fn set_sample(&mut self, buffer: Arc<SampleBuffer>) {
        self.sample_buffer = Some(buffer);
    }

    /// Triggers note playback at a given frequency and velocity.
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        self.triggered_freq = frequency;
        self.velocity = velocity;
        self.phase = 0.0;
        
        for grain in &mut self.grains {
            grain.active = false;
        }
        
        if let Some(ref buffer) = self.sample_buffer {
            let grain_size_samples = (self.grain_size_ms / 1000.0) * buffer.sample_rate;
            let spawn_interval = grain_size_samples / self.overlap.max(1) as f32;
            self.grain_spawn_timer = spawn_interval;
        }

        self.active = self.sample_buffer.is_some();
    }

    /// Releases the active note.
    pub fn note_off(&mut self) {
        if self.play_mode == PlayMode::Loop {
            self.active = false;
        }
    }

    /// Generates a single stereo frame of sample playback.
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

        match self.stretch_mode {
            StretchMode::Resample => {
                let rate_multiplier = self.pitch_ratio * self.speed_ratio;
                let phase_increment = (original_sample_rate as f64 / self.sample_rate as f64) * rate_multiplier as f64;

                let index = self.phase;
                let index_floor = index.floor() as usize;
                let index_next = index_floor + 1;
                let frac = (index - index_floor as f64) as f32;

                let mut out_l = 0.0;
                let mut out_r = 0.0;

                if channels == 1 {
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
                    let base0 = (index_floor % num_frames) * channels;
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
            StretchMode::Granular => {
                let grain_size_samples = (self.grain_size_ms / 1000.0) * original_sample_rate;
                let spawn_interval = grain_size_samples / self.overlap.max(1) as f32;

                // Spawn logic
                self.grain_spawn_timer += 1.0;
                if self.grain_spawn_timer >= spawn_interval {
                    self.grain_spawn_timer = 0.0;
                    
                    let can_spawn = match self.play_mode {
                        PlayMode::Loop => true,
                        PlayMode::OneShot => self.phase < num_frames as f64,
                    };

                    if can_spawn {
                        if let Some(grain) = self.grains.iter_mut().find(|g| !g.active) {
                            grain.active = true;
                            grain.phase = 0.0;
                            grain.source_start = self.phase;
                        }
                    }
                }

                let mut out_l = 0.0;
                let mut out_r = 0.0;
                let mut active_grains_count = 0;

                for grain in &mut self.grains {
                    if !grain.active {
                        continue;
                    }
                    active_grains_count += 1;

                    let read_pos = grain.source_start + grain.phase;

                    // Window function (Hanning)
                    let frac = (grain.phase / grain_size_samples as f64) as f32;
                    let window = 0.5 * (1.0 - (frac * 2.0 * std::f32::consts::PI).cos());

                    let index_floor = read_pos.floor() as usize;
                    let index_next = index_floor + 1;
                    let interp_frac = (read_pos - index_floor as f64) as f32;

                    let mut g_l = 0.0;
                    let mut g_r = 0.0;

                    if channels == 1 {
                        let s0 = data[index_floor % num_frames];
                        let s1 = if index_next < num_frames {
                            data[index_next]
                        } else if self.play_mode == PlayMode::Loop {
                            data[index_next % num_frames]
                        } else {
                            0.0
                        };
                        let sample = s0 + (s1 - s0) * interp_frac;
                        g_l = sample * window;
                        g_r = sample * window;
                    } else if channels >= 2 {
                        let base0 = (index_floor % num_frames) * channels;
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

                        g_l = (s0_l + (s1_l - s0_l) * interp_frac) * window;
                        g_r = (s0_r + (s1_r - s0_r) * interp_frac) * window;
                    }

                    out_l += g_l;
                    out_r += g_r;

                    let pitch_inc = (original_sample_rate as f64 / self.sample_rate as f64) * self.pitch_ratio as f64;
                    grain.phase += pitch_inc;

                    if grain.phase >= grain_size_samples as f64 {
                        grain.active = false;
                    }
                }

                let playhead_inc = (original_sample_rate as f64 / self.sample_rate as f64) * self.speed_ratio as f64;
                self.phase += playhead_inc;

                if self.phase >= num_frames as f64 {
                    match self.play_mode {
                        PlayMode::Loop => {
                            self.phase -= num_frames as f64;
                        }
                        PlayMode::OneShot => {
                            if active_grains_count == 0 {
                                self.active = false;
                            }
                        }
                    }
                }

                (out_l * self.velocity, out_r * self.velocity)
            }
        }
    }
}

/// The multi-voice sampler manager loaded as an Instrument.
pub struct Sampler {
    /// Array of voices available for polyphony.
    pub voices: Vec<SamplerVoice>,
    /// Shared audio sample buffer.
    pub sample_buffer: Option<Arc<SampleBuffer>>,
    /// Playback loop or one-shot mode.
    pub play_mode: PlayMode,
    /// Pitch shifting factor.
    pub pitch_ratio: f32,
    /// Playback speed factor.
    pub speed_ratio: f32,
    /// MIDI note reference frequency (root key).
    pub root_freq: f32,

    /// Time stretch engine mode.
    pub stretch_mode: StretchMode,
    /// Grain duration size in milliseconds.
    pub grain_size_ms: f32,
    /// Overlapping grain count.
    pub overlap: usize,
}

impl Sampler {
    /// Instantiates a new Sampler containing `num_voices` polyphonic voices.
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
            root_freq: 261.63,
            stretch_mode: StretchMode::Resample,
            grain_size_ms: 40.0,
            overlap: 4,
        }
    }

    /// Sets the underlying audio sample buffer.
    pub fn set_sample(&mut self, buffer: SampleBuffer) {
        let arc_buf = Arc::new(buffer);
        self.sample_buffer = Some(arc_buf.clone());
        for voice in &mut self.voices {
            voice.set_sample(arc_buf.clone());
        }
    }

    /// Configures the sampler playback mode.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
        for voice in &mut self.voices {
            voice.play_mode = mode;
        }
    }

    /// Configures the pitch ratio.
    pub fn set_pitch_ratio(&mut self, ratio: f32) {
        self.pitch_ratio = ratio.max(0.001);
        for voice in &mut self.voices {
            voice.pitch_ratio = self.pitch_ratio * (voice.triggered_freq / self.root_freq);
        }
    }

    /// Configures the playback speed ratio.
    pub fn set_speed_ratio(&mut self, ratio: f32) {
        self.speed_ratio = ratio.max(0.001);
        for voice in &mut self.voices {
            voice.speed_ratio = self.speed_ratio;
        }
    }

    /// Configures the time-stretch engine mode.
    pub fn set_stretch_mode(&mut self, mode: StretchMode) {
        self.stretch_mode = mode;
        for voice in &mut self.voices {
            voice.stretch_mode = mode;
        }
    }

    /// Configures the grain duration in milliseconds.
    pub fn set_grain_size(&mut self, size_ms: f32) {
        self.grain_size_ms = size_ms.clamp(5.0, 500.0);
        for voice in &mut self.voices {
            voice.grain_size_ms = self.grain_size_ms;
        }
    }

    /// Configures the grain overlap factor.
    pub fn set_overlap(&mut self, overlap: usize) {
        self.overlap = overlap.clamp(1, 16);
        for voice in &mut self.voices {
            voice.overlap = self.overlap;
        }
    }

    /// Triggers note playback. Attempts to find a free voice or steals voice 0.
    pub fn note_on(&mut self, frequency: f32, velocity: f32) {
        if let Some(voice) = self.voices.iter_mut().find(|v| !v.active) {
            voice.play_mode = self.play_mode;
            voice.speed_ratio = self.speed_ratio;
            voice.pitch_ratio = self.pitch_ratio * (frequency / self.root_freq);
            voice.stretch_mode = self.stretch_mode;
            voice.grain_size_ms = self.grain_size_ms;
            voice.overlap = self.overlap;
            voice.note_on(frequency, velocity);
        } else {
            let voice = &mut self.voices[0];
            voice.play_mode = self.play_mode;
            voice.speed_ratio = self.speed_ratio;
            voice.pitch_ratio = self.pitch_ratio * (frequency / self.root_freq);
            voice.stretch_mode = self.stretch_mode;
            voice.grain_size_ms = self.grain_size_ms;
            voice.overlap = self.overlap;
            voice.note_on(frequency, velocity);
        }
    }

    /// Releases active note frequencies matching `frequency`.
    pub fn note_off(&mut self, frequency: f32) {
        for voice in &mut self.voices {
            if (voice.triggered_freq - frequency).abs() < 0.01 && voice.active {
                voice.note_off();
            }
        }
    }

    /// Immediately silences all active sampler voices.
    pub fn note_off_all(&mut self) {
        for voice in &mut self.voices {
            voice.active = false;
        }
    }

    /// Sums polyphonic voice outputs and applies headroom gain.
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

    #[test]
    fn test_sampler_voice_granular() {
        let mut voice = SamplerVoice::new(44100.0);
        voice.stretch_mode = StretchMode::Granular;
        voice.grain_size_ms = 10.0;
        voice.overlap = 2;

        let buffer = SampleBuffer {
            data: vec![1.0; 1000],
            channels: 1,
            sample_rate: 44100.0,
        };
        voice.set_sample(Arc::new(buffer));
        voice.note_on(261.63, 1.0);

        assert!(voice.active);

        let mut produced_audio = false;
        for _ in 0..10 {
            let (l, r) = voice.process();
            if l > 0.0 && r > 0.0 {
                produced_audio = true;
                break;
            }
        }
        assert!(produced_audio);
    }
}
