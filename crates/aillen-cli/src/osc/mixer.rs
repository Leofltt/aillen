use rosc::OscMessage;
use crossbeam_channel::Sender;
use crate::audio::AudioMessage;

pub fn parse_mixer_command(addr: &str, msg: &OscMessage, prod: &Sender<AudioMessage>) {
    if addr == "/mixer/master/volume" {
        if let Some(vol) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetMasterVolume { volume: vol });
        }
    } else if addr == "/mixer/master/filter" {
        if let Some(pos) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::MixerSetMasterFilter { position: pos });
        }
    } else if addr == "/mixer/master/waveloss/drop" {
        if let Some(arg) = msg.args.get(0) {
            let drop = arg.clone().int().unwrap_or(0) as usize;
            let _ = prod.try_send(AudioMessage::SetMasterWlDrop { drop });
        }
    } else if addr == "/mixer/master/waveloss/outof" {
        if let Some(arg) = msg.args.get(0) {
            let outof = arg.clone().int().unwrap_or(40) as usize;
            let _ = prod.try_send(AudioMessage::SetMasterWlOutof { outof });
        }
    } else if addr == "/mixer/master/waveloss/mode" {
        if let Some(arg) = msg.args.get(0) {
            let mode = arg.clone().int().unwrap_or(1) as usize;
            let _ = prod.try_send(AudioMessage::SetMasterWlMode { mode });
        }
    } else if addr == "/mixer/master/limiter/gain" {
        if let Some(gain) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetMasterLimiterGain { gain });
        }
    } else if addr == "/mixer/master/limiter/release" {
        if let Some(rel) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetMasterLimiterRelease { release: rel });
        }
    } else if addr == "/mixer/master/limiter/ceiling" {
        if let Some(ceil) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetMasterLimiterCeiling { ceiling: ceil });
        }
    } else if addr == "/mixer/return/delay/time" {
        if let Some(time) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetReturnDelayTime { time });
        }
    } else if addr == "/mixer/return/delay/feedback" {
        if let Some(feedback) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetReturnDelayFeedback { feedback });
        }
    } else if addr == "/mixer/return/delay/mode" {
        if let Some(arg) = msg.args.get(0) {
            let mode = arg.clone().int().unwrap_or(0) as usize;
            let _ = prod.try_send(AudioMessage::SetReturnDelayMode { mode });
        }
    } else if addr == "/mixer/return/delay/pingpong" {
        if let Some(arg) = msg.args.get(0) {
            let enabled = match arg {
                rosc::OscType::Bool(b) => *b,
                rosc::OscType::Int(i) => *i > 0,
                rosc::OscType::Float(f) => *f > 0.5,
                _ => false,
            };
            let _ = prod.try_send(AudioMessage::SetReturnDelayPingPong { enabled });
        }
    } else if addr == "/mixer/return/delay/drive" {
        if let Some(drive) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetReturnDelayDrive { drive });
        }
    } else if addr == "/mixer/return/delay/grain_size" {
        if let Some(size) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetReturnDelayGrainSize { size });
        }
    } else if addr == "/mixer/return/delay/density" {
        if let Some(arg) = msg.args.get(0) {
            let density = arg.clone().int().unwrap_or(4) as usize;
            let _ = prod.try_send(AudioMessage::SetReturnDelayDensity { density });
        }
    } else if addr == "/mixer/return/delay/spray" {
        if let Some(spray) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetReturnDelaySpray { spray });
        }
    } else if addr == "/mixer/return/delay/pitch" {
        if let Some(pitch) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetReturnDelayPitch { pitch });
        }
    }
}
