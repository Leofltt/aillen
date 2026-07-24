use std::fs::File;
use std::path::Path;
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
    
    // Peak normalization
    let mut peak = 0.0f32;
    for &sample in &data {
        peak = peak.max(sample.abs());
    }
    if peak > 0.0 {
        let gain = 1.0 / peak;
        for sample in &mut data {
            *sample *= gain;
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
