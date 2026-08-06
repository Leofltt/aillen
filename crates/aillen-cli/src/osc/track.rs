use rosc::OscMessage;
use crossbeam_channel::Sender;
use crate::audio::AudioMessage;
use aillen_core::sample_bank::SampleBank;
use aillen_core::dsp::{oscillator::Waveform, filter::FilterType};
use aillen_core::synth::two_op::SynthMode;
use aillen_core::synth::sampler::{PlayMode, StretchMode};

fn osc_to_usize(arg: &rosc::OscType) -> Option<usize> {
    match arg {
        rosc::OscType::Int(i) => Some(*i as usize),
        rosc::OscType::Float(f) => Some(*f as usize),
        rosc::OscType::Double(d) => Some(*d as usize),
        _ => None,
    }
}

pub fn parse_track_command(
    track_id: usize,
    sub_addr: &str,
    msg: &OscMessage,
    prod: &Sender<AudioMessage>,
    bank: &SampleBank,
) {
    match sub_addr {
        "note/on" => {
            if let Some(freq) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let vel = msg.args.get(1).and_then(|a| a.clone().float()).unwrap_or(1.0);
                let _ = prod.try_send(AudioMessage::TrackNoteOn { track_id, freq, vel });
            }
        }
        "note/off" => {
            if let Some(freq) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::TrackNoteOff { track_id, freq });
            } else {
                let _ = prod.try_send(AudioMessage::TrackNoteOffAll { track_id });
            }
        }
        "note" => {
            if msg.args.len() >= 2 {
                let freq = msg.args[0].clone().float().unwrap_or(440.0);
                let duration_ms = msg.args[1].clone().float().unwrap_or(100.0);
                let vel = msg.args.get(2).and_then(|a| a.clone().float()).unwrap_or(1.0);
                let _ = prod.try_send(AudioMessage::TrackTimedNote { track_id, freq, vel, duration_ms });
            }
        }
        "volume" => {
            if let Some(vol) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackVolume { track_id, volume: vol });
            }
        }
        "pan" => {
            if let Some(pan) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackPan { track_id, pan });
            }
        }
        "mute" => {
            if let Some(arg) = msg.args.get(0) {
                let mute = match arg {
                    rosc::OscType::Bool(b) => *b,
                    rosc::OscType::Int(i) => *i > 0,
                    rosc::OscType::Float(f) => *f > 0.5,
                    _ => false,
                };
                let _ = prod.try_send(AudioMessage::SetTrackMute { track_id, mute });
            }
        }
        "send/delay" => {
            if let Some(send) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackSendDelay { track_id, send });
            }
        }
        "send/reverb" => {
            if let Some(send) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackSendReverb { track_id, send });
            }
        }
        // FX Chain commands
        "fx/filter/position" => {
            if let Some(pos) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackFxFilterPos { track_id, pos });
            }
        }
        "fx/ring_mod/mode" => {
            if let Some(arg) = msg.args.get(0) {
                let ring_mod = match arg {
                    rosc::OscType::Bool(b) => *b,
                    rosc::OscType::Int(i) => *i > 0,
                    rosc::OscType::Float(f) => *f > 0.5,
                    _ => false,
                };
                let _ = prod.try_send(AudioMessage::SetTrackRmMode { track_id, ring_mod });
            }
        }
        "fx/ring_mod/source" => {
            if let Some(arg) = msg.args.get(0) {
                let source = arg.clone().int().unwrap_or(0) as usize;
                let _ = prod.try_send(AudioMessage::SetTrackRmSource { track_id, source });
            }
        }
        "fx/ring_mod/depth" => {
            if let Some(depth) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackRmDepth { track_id, depth });
            }
        }
        "fx/ring_mod/freq" => {
            if let Some(freq) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackRmFreq { track_id, freq });
            }
        }
        "fx/compressor/ratio" => {
            if let Some(ratio) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCompRatio { track_id, ratio });
            }
        }
        "fx/compressor/threshold" => {
            if let Some(thresh) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCompThreshold { track_id, thresh });
            }
        }
        "fx/compressor/attack" => {
            if let Some(attack) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCompAttack { track_id, attack });
            }
        }
        "fx/compressor/release" => {
            if let Some(release) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCompRelease { track_id, release });
            }
        }
        "fx/compressor/makeup" => {
            if let Some(makeup) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCompMakeup { track_id, makeup });
            }
        }
        "fx/compressor/sidechain" => {
            if let Some(arg) = msg.args.get(0) {
                let enabled = match arg {
                    rosc::OscType::Bool(b) => *b,
                    rosc::OscType::Int(i) => *i > 0,
                    rosc::OscType::Float(f) => *f > 0.5,
                    _ => false,
                };
                let _ = prod.try_send(AudioMessage::SetTrackCompSidechain { track_id, enabled });
            }
        }
        "sidechain/source" => {
            if let Some(arg) = msg.args.get(0) {
                let src_idx = arg.clone().int().unwrap_or(-1);
                let source_id = if src_idx >= 0 { Some(src_idx as usize) } else { None };
                let _ = prod.try_send(AudioMessage::SetTrackSidechainSource { track_id, source_id });
            }
        }
        "fx/distortion/mode" => {
            if let Some(mode) = msg.args.get(0).and_then(|a| a.clone().int()) {
                let _ = prod.try_send(AudioMessage::SetTrackDistortionMode { track_id, mode });
            }
        }
        "fx/distortion/drive" => {
            if let Some(drive) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackDistortionDrive { track_id, drive });
            }
        }
        "fx/distortion/mix" => {
            if let Some(mix) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackDistortionMix { track_id, mix });
            }
        }
        "fx/wavefolder/drive" => {
            if let Some(drive) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackWfDrive { track_id, drive });
            }
        }
        "fx/wavefolder/folds" => {
            if let Some(folds) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackWfFolds { track_id, folds });
            }
        }
        "fx/wavefolder/symmetry" => {
            if let Some(symmetry) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackWfSymmetry { track_id, symmetry });
            }
        }
        "fx/bitcrusher/bits" => {
            if let Some(bits) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackBitcrusherBits { track_id, bits });
            }
        }
        "fx/bitcrusher/downsample" => {
            if let Some(downsample) = msg.args.get(0).and_then(|a| a.clone().int()) {
                let _ = prod.try_send(AudioMessage::SetTrackBitcrusherDownsample { track_id, downsample: downsample.max(1) as usize });
            }
        }
        "fx/comb/freq" => {
            if let Some(freq) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCombFreq { track_id, freq });
            }
        }
        "fx/comb/feedback" => {
            if let Some(feedback) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCombFeedback { track_id, feedback });
            }
        }
        "fx/comb/damp" => {
            if let Some(damp) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SetTrackCombDamp { track_id, damp });
            }
        }
        // Two-Op specific (Track 0 and Track 4)
        "legato" if track_id == 0 || track_id == 4 => {
            if let Some(arg) = msg.args.get(0) {
                let enabled = match arg {
                    rosc::OscType::Bool(b) => *b, rosc::OscType::Int(i) => *i > 0, rosc::OscType::Float(f) => *f > 0.5, _ => false,
                };
                let _ = prod.try_send(AudioMessage::TwoOpSetLegato { track_id, enabled });
            }
        }
        "realtime" if track_id == 0 || track_id == 4 => {
            if let Some(arg) = msg.args.get(0) {
                let enabled = match arg {
                    rosc::OscType::Bool(b) => *b, rosc::OscType::Int(i) => *i > 0, rosc::OscType::Float(f) => *f > 0.5, _ => false,
                };
                let _ = prod.try_send(AudioMessage::TwoOpSetRealtimeUpdate { track_id, enabled });
            }
        }
        "mode" if track_id == 0 || track_id == 4 => {
            if let Some(arg) = msg.args.get(0) {
                let mode_idx = arg.clone().int().unwrap_or(0);
                let mode = match mode_idx {
                    0 => SynthMode::Additive, 1 => SynthMode::Am, 2 => SynthMode::Rm, 3 => SynthMode::Fm, _ => SynthMode::Additive,
                };
                let _ = prod.try_send(AudioMessage::TwoOpSetMode { track_id, mode });
            }
        }
        "osc1/waveform" if track_id == 0 || track_id == 4 => {
            if let Some(arg) = msg.args.get(0) {
                let wf_idx = arg.clone().int().unwrap_or(0);
                let waveform = match wf_idx {
                    0 => Waveform::Sine, 1 => Waveform::Saw, 2 => Waveform::Square, 3 => Waveform::Triangle, _ => Waveform::Sine,
                };
                let _ = prod.try_send(AudioMessage::TwoOpSetOsc1Waveform { track_id, waveform });
            }
        }
        "osc2/waveform" if track_id == 0 || track_id == 4 => {
            if let Some(arg) = msg.args.get(0) {
                let wf_idx = arg.clone().int().unwrap_or(0);
                let waveform = match wf_idx {
                    0 => Waveform::Sine, 1 => Waveform::Saw, 2 => Waveform::Square, 3 => Waveform::Triangle, _ => Waveform::Sine,
                };
                let _ = prod.try_send(AudioMessage::TwoOpSetOsc2Waveform { track_id, waveform });
            }
        }
        "osc1/adsr" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 4 {
                let a = msg.args[0].clone().float().unwrap_or(0.01);
                let d = msg.args[1].clone().float().unwrap_or(0.1);
                let s = msg.args[2].clone().float().unwrap_or(0.5);
                let r = msg.args[3].clone().float().unwrap_or(0.5);
                let _ = prod.try_send(AudioMessage::TwoOpSetOsc1Adsr { track_id, a, d, s, r });
            }
        }
        "osc2/adsr" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 4 {
                let a = msg.args[0].clone().float().unwrap_or(0.01);
                let d = msg.args[1].clone().float().unwrap_or(0.1);
                let s = msg.args[2].clone().float().unwrap_or(0.5);
                let r = msg.args[3].clone().float().unwrap_or(0.5);
                let _ = prod.try_send(AudioMessage::TwoOpSetOsc2Adsr { track_id, a, d, s, r });
            }
        }
        "filter/adsr" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 4 {
                let a = msg.args[0].clone().float().unwrap_or(0.01);
                let d = msg.args[1].clone().float().unwrap_or(0.1);
                let s = msg.args[2].clone().float().unwrap_or(0.5);
                let r = msg.args[3].clone().float().unwrap_or(0.5);
                let _ = prod.try_send(AudioMessage::TwoOpSetFilterAdsr { track_id, a, d, s, r });
            }
        }
        "filter/params" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 3 {
                let cutoff = msg.args[0].clone().float().unwrap_or(1000.0);
                let q = msg.args[1].clone().float().unwrap_or(0.707);
                let type_idx = msg.args[2].clone().int().unwrap_or(0);
                let filter_type = match type_idx {
                    0 => FilterType::LowPass, 1 => FilterType::HighPass, 2 => FilterType::BandPass, 3 => FilterType::Notch, _ => FilterType::LowPass,
                };
                let _ = prod.try_send(AudioMessage::TwoOpSetFilterParams { track_id, cutoff, q, filter_type });
            }
        }
        "filter/mod" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 2 {
                let enabled = match msg.args[0] {
                    rosc::OscType::Bool(b) => b, rosc::OscType::Int(i) => i > 0, rosc::OscType::Float(f) => f > 0.5, _ => false,
                };
                let amount = msg.args[1].clone().float().unwrap_or(0.0);
                let _ = prod.try_send(AudioMessage::TwoOpSetFilterMod { track_id, enabled, amount });
            }
        }
        "mod/params" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 3 {
                let index = msg.args[0].clone().float().unwrap_or(1.0);
                let ratio = msg.args[1].clone().float().unwrap_or(1.0);
                let detune = msg.args[2].clone().float().unwrap_or(0.0);
                let _ = prod.try_send(AudioMessage::TwoOpSetModulationParams { track_id, index, ratio, detune });
            }
        }
        "feedback" | "twoop/feedback" if track_id == 0 || track_id == 4 => {
            if let Some(feedback) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::TwoOpSetOsc2Feedback { track_id, feedback });
            }
        }
        "wavefold" | "twoop/wavefold" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 2 {
                let gain = msg.args[0].clone().float().unwrap_or(1.0);
                let mix = msg.args[1].clone().float().unwrap_or(0.0);
                let _ = prod.try_send(AudioMessage::TwoOpSetWavefold { track_id, gain, mix });
            }
        }
        "noise" | "twoop/noise" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 2 {
                let carrier_noise = msg.args[0].clone().float().unwrap_or(0.0);
                let modulator_noise = msg.args[1].clone().float().unwrap_or(0.0);
                let _ = prod.try_send(AudioMessage::TwoOpSetNoise { track_id, carrier_noise, modulator_noise });
            }
        }
        "pitch/sweep" | "twoop/pitch/sweep" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 2 {
                let depth = msg.args[0].clone().float().unwrap_or(0.0);
                let decay = msg.args[1].clone().float().unwrap_or(0.1);
                let _ = prod.try_send(AudioMessage::TwoOpSetPitchSweep { track_id, depth, decay });
            }
        }
        "lfo" | "twoop/lfo" if track_id == 0 || track_id == 4 => {
            if msg.args.len() >= 4 {
                let waveform = msg.args[0].clone().int().unwrap_or(0) as usize;
                let speed = msg.args[1].clone().float().unwrap_or(2.0);
                let mod_index = msg.args[2].clone().float().unwrap_or(0.0);
                let cutoff = msg.args[3].clone().float().unwrap_or(0.0);
                let _ = prod.try_send(AudioMessage::TwoOpSetLfo { track_id, waveform, speed, mod_index, cutoff });
            }
        }
        // Sampler specific (Tracks 1, 2, 3, 5)
        "sample/load" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(path) = msg.args.get(0).and_then(|a| a.clone().string()) {
                let mut found_in_bank = None;
                for (key, buffer) in &bank.samples {
                    if path.ends_with(key) {
                        found_in_bank = Some(buffer.clone());
                        break;
                    }
                }
                if let Some(buffer) = found_in_bank {
                    println!("SampleBank: Match found for \"{}\", loading from cache!", path);
                    let _ = prod.try_send(AudioMessage::SamplerLoadBuffer { track_id, buffer });
                } else {
                    let _ = prod.try_send(AudioMessage::SamplerLoadSample { track_id, path });
                }
            }
        }
        "sample/select" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(name) = msg.args.get(0).and_then(|a| a.clone().string()) {
                if let Some(buffer) = bank.get(&name) {
                    let _ = prod.try_send(AudioMessage::SamplerLoadBuffer { track_id, buffer });
                } else {
                    eprintln!("SampleBank: Sample not found: \"{}\"", name);
                }
            }
        }
        "sample/mode" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(arg) = msg.args.get(0) {
                let mode_idx = arg.clone().int().unwrap_or(0);
                let mode = match mode_idx {
                    0 => PlayMode::OneShot,
                    1 => PlayMode::Loop,
                    _ => PlayMode::OneShot,
                };
                let _ = prod.try_send(AudioMessage::SamplerSetPlayMode { track_id, mode });
            }
        }
        "sample/pitch" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(ratio) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SamplerSetPitchRatio { track_id, ratio });
            }
        }
        "sample/speed" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(ratio) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SamplerSetSpeedRatio { track_id, ratio });
            }
        }
        "sample/mode/stretch" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(arg) = msg.args.get(0) {
                let mode_idx = arg.clone().int().unwrap_or(0);
                let mode = match mode_idx {
                    0 => StretchMode::Resample,
                    1 => StretchMode::Granular,
                    _ => StretchMode::Resample,
                };
                let _ = prod.try_send(AudioMessage::SamplerSetStretchMode { track_id, mode });
            }
        }
        "sample/grain_size" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(size_ms) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SamplerSetGrainSize { track_id, size_ms });
            }
        }
        "sample/overlap" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(arg) = msg.args.get(0) {
                let overlap = osc_to_usize(arg).unwrap_or(4);
                let _ = prod.try_send(AudioMessage::SamplerSetOverlap { track_id, overlap });
            }
        }
        "filter" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(pos) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SamplerSetDjFilter { track_id, position: pos });
            }
        }
        "sample/slice/mode" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(arg) = msg.args.get(0) {
                let enabled = match arg {
                    rosc::OscType::Bool(b) => *b,
                    rosc::OscType::Int(i) => *i > 0,
                    rosc::OscType::Float(f) => *f > 0.5,
                    _ => false,
                };
                let _ = prod.try_send(AudioMessage::SamplerSetSliceMode { track_id, enabled });
            }
        }
        "sample/slice/count" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(arg) = msg.args.get(0) {
                let n = osc_to_usize(arg).unwrap_or(16);
                let _ = prod.try_send(AudioMessage::SamplerSetNumSlices { track_id, n });
            }
        }
        "sample/slice/select" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(arg) = msg.args.get(0) {
                let slice = osc_to_usize(arg).unwrap_or(0);
                let _ = prod.try_send(AudioMessage::SamplerSetSelectedSlice { track_id, slice });
            }
        }
        "sample/slice/stutter" if track_id == 1 || track_id == 2 || track_id == 3 || track_id == 5 => {
            if let Some(arg) = msg.args.get(0) {
                let count = osc_to_usize(arg).unwrap_or(1);
                let _ = prod.try_send(AudioMessage::SamplerSetStutterCount { track_id, count });
            }
        }
        // Synth303 specific (Track 6)
        "303/waveform" if track_id == 6 => {
            if let Some(arg) = msg.args.get(0) {
                let wf_idx = arg.clone().int().unwrap_or(1);
                let waveform = match wf_idx {
                    0 => Waveform::Sine, 1 => Waveform::Saw, 2 => Waveform::Square, 3 => Waveform::Triangle, _ => Waveform::Saw,
                };
                let _ = prod.try_send(AudioMessage::Synth303SetWaveform { waveform });
            }
        }
        "303/amp/adsr" if track_id == 6 => {
            if msg.args.len() >= 4 {
                let a = msg.args[0].clone().float().unwrap_or(0.002);
                let d = msg.args[1].clone().float().unwrap_or(0.3);
                let s = msg.args[2].clone().float().unwrap_or(0.1);
                let r = msg.args[3].clone().float().unwrap_or(0.2);
                let _ = prod.try_send(AudioMessage::Synth303SetAmpAdsr { a, d, s, r });
            }
        }
        "303/filter/adsr" if track_id == 6 => {
            if msg.args.len() >= 4 {
                let a = msg.args[0].clone().float().unwrap_or(0.002);
                let d = msg.args[1].clone().float().unwrap_or(0.25);
                let s = msg.args[2].clone().float().unwrap_or(0.05);
                let r = msg.args[3].clone().float().unwrap_or(0.2);
                let _ = prod.try_send(AudioMessage::Synth303SetFilterAdsr { a, d, s, r });
            }
        }
        "303/pitch/adsr" if track_id == 6 => {
            if msg.args.len() >= 4 {
                let a = msg.args[0].clone().float().unwrap_or(0.002);
                let d = msg.args[1].clone().float().unwrap_or(0.1);
                let s = msg.args[2].clone().float().unwrap_or(0.0);
                let r = msg.args[3].clone().float().unwrap_or(0.1);
                let _ = prod.try_send(AudioMessage::Synth303SetPitchAdsr { a, d, s, r });
            }
        }
        "303/filter/params" if track_id == 6 => {
            if msg.args.len() >= 2 {
                let cutoff = msg.args[0].clone().float().unwrap_or(300.0);
                let resonance = msg.args[1].clone().float().unwrap_or(0.75);
                let _ = prod.try_send(AudioMessage::Synth303SetFilterParams { cutoff, resonance });
            }
        }
        "303/filter/mod" if track_id == 6 => {
            if let Some(amount) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::Synth303SetFilterMod { amount });
            }
        }
        "303/pitch/mod" if track_id == 6 => {
            if let Some(amount) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::Synth303SetPitchMod { amount });
            }
        }
        "303/pwm/params" if track_id == 6 => {
            if msg.args.len() >= 3 {
                let pw = msg.args[0].clone().float().unwrap_or(0.5);
                let rate = msg.args[1].clone().float().unwrap_or(1.0);
                let depth = msg.args[2].clone().float().unwrap_or(0.0);
                let _ = prod.try_send(AudioMessage::Synth303SetPwmParams { pw, rate, depth });
            }
        }
        "303/glide" if track_id == 6 => {
            if let Some(seconds) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::Synth303SetGlideTime { seconds });
            }
        }
        "303/legato" if track_id == 6 => {
            if let Some(arg) = msg.args.get(0) {
                let enabled = match arg {
                    rosc::OscType::Bool(b) => *b, rosc::OscType::Int(i) => *i > 0, rosc::OscType::Float(f) => *f > 0.5, _ => false,
                };
                let _ = prod.try_send(AudioMessage::Synth303SetLegato { enabled });
            }
        }
        "hubass/amp/adsr" if track_id == 7 => {
            if msg.args.len() >= 4 {
                let a = msg.args[0].clone().float().unwrap_or(0.01);
                let d = msg.args[1].clone().float().unwrap_or(0.1);
                let s = msg.args[2].clone().float().unwrap_or(1.0);
                let r = msg.args[3].clone().float().unwrap_or(0.1);
                let _ = prod.try_send(AudioMessage::SynthHubassSetAmpAdsr { a, d, s, r });
            }
        }
        "hubass/filter/params" if track_id == 7 => {
            if msg.args.len() >= 4 {
                let start_mult = msg.args[0].clone().float().unwrap_or(1.333);
                let end_cf = msg.args[1].clone().float().unwrap_or(800.0);
                let decay = msg.args[2].clone().float().unwrap_or(1.0);
                let resonance = msg.args[3].clone().float().unwrap_or(0.4);
                let _ = prod.try_send(AudioMessage::SynthHubassSetFilterParams { start_mult, end_cf, decay, resonance });
            }
        }
        "hubass/osc/unison" if track_id == 7 => {
            if msg.args.len() >= 4 {
                let waveform = msg.args[0].clone().int().unwrap_or(0);
                let detune = msg.args[1].clone().float().unwrap_or(0.035);
                let spread = msg.args[2].clone().float().unwrap_or(0.8);
                let num_voices = msg.args[3].clone().int().unwrap_or(5);
                let _ = prod.try_send(AudioMessage::SynthHubassSetOscUnison { waveform, detune, spread, num_voices });
            }
        }
        "hubass/osc/sub" if track_id == 7 => {
            if msg.args.len() >= 3 {
                let waveform = msg.args[0].clone().int().unwrap_or(0);
                let octave_offset = msg.args[1].clone().int().unwrap_or(-1);
                let gain = msg.args[2].clone().float().unwrap_or(0.7);
                let _ = prod.try_send(AudioMessage::SynthHubassSetOscSub { waveform, octave_offset, gain });
            }
        }
        "hubass/osc/noise" if track_id == 7 => {
            if let Some(gain) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SynthHubassSetOscNoise { gain });
            }
        }
        "hubass/filter/mode" if track_id == 7 => {
            if let Some(mode) = msg.args.get(0).and_then(|a| a.clone().int()) {
                let _ = prod.try_send(AudioMessage::SynthHubassSetFilterMode { mode });
            }
        }
        "hubass/drive/mode" if track_id == 7 => {
            if msg.args.len() >= 3 {
                let mode = msg.args[0].clone().int().unwrap_or(1);
                let gain = msg.args[1].clone().float().unwrap_or(2.0);
                let mix = msg.args[2].clone().float().unwrap_or(0.5);
                let _ = prod.try_send(AudioMessage::SynthHubassSetDriveMode { mode, gain, mix });
            }
        }
        "hubass/lfo/1" if track_id == 7 => {
            if msg.args.len() >= 4 {
                let waveform = msg.args[0].clone().int().unwrap_or(0);
                let speed_hz = msg.args[1].clone().float().unwrap_or(1.5);
                let cutoff_depth = msg.args[2].clone().float().unwrap_or(0.0);
                let pitch_depth = msg.args[3].clone().float().unwrap_or(0.0);
                let _ = prod.try_send(AudioMessage::SynthHubassSetLfo1 { waveform, speed_hz, cutoff_depth, pitch_depth });
            }
        }
        "hubass/chorus/params" if track_id == 7 => {
            if msg.args.len() >= 2 {
                let mix = msg.args[0].clone().float().unwrap_or(0.6);
                let depth = msg.args[1].clone().float().unwrap_or(0.5);
                let _ = prod.try_send(AudioMessage::SynthHubassSetChorusParams { mix, depth });
            }
        }
        "hubass/legato" if track_id == 7 => {
            if let Some(arg) = msg.args.get(0) {
                let enabled = match arg {
                    rosc::OscType::Bool(b) => *b, rosc::OscType::Int(i) => *i > 0, rosc::OscType::Float(f) => *f > 0.5, _ => false,
                };
                let _ = prod.try_send(AudioMessage::SynthHubassSetLegato { enabled });
            }
        }
        "hubass/gain" if track_id == 7 => {
            if let Some(gain) = msg.args.get(0).and_then(|a| a.clone().float()) {
                let _ = prod.try_send(AudioMessage::SynthHubassSetOutputGain { gain });
            }
        }
        _ => {}
    }
}
