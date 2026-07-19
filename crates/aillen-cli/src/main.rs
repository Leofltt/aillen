mod audio;

use clap::Parser;
use std::net::UdpSocket;
use rosc::{OscPacket, OscMessage};
use crossbeam_channel::{bounded, Sender};
use audio::{AudioMessage, start_audio_thread};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 8000)]
    port: u16,
    #[arg(short, long, default_value_t = 8)]
    voices: usize,
    #[arg(short, long)]
    device_index: Option<usize>,
    #[arg(short, long)]
    list_devices: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.list_devices {
        audio::list_audio_devices()?;
        return Ok(());
    }
    println!("Aillen Synthesizer CLI Starting up...");
    let (prod, cons) = bounded::<AudioMessage>(256);
    let _stream = start_audio_thread(cons, args.voices, args.device_index)?;
    println!("Audio thread created successfully.");
    let socket = UdpSocket::bind(&format!("0.0.0.0:{}", args.port))?;
    println!("Listening for OSC messages on UDP port {}", args.port);
    let mut buf = [0u8; 2048];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _client_addr)) => {
                let packet_slice = &buf[..size];
                match rosc::decoder::decode_udp(packet_slice) {
                    Ok((_, OscPacket::Message(msg))) => handle_osc_message(msg, &prod),
                    Ok((_, OscPacket::Bundle(bundle))) => {
                        for packet in bundle.content {
                            if let OscPacket::Message(msg) = packet {
                                handle_osc_message(msg, &prod);
                            }
                        }
                    }
                    Err(e) => eprintln!("Failed to decode OSC packet: {:?}", e),
                }
            }
            Err(e) => eprintln!("Socket receive error: {}", e),
        }
    }
}

use aillen_core::dsp::{oscillator::Waveform, filter::FilterType};
use aillen_core::synth::two_op::SynthMode;
use aillen_core::synth::sampler::PlayMode;

fn handle_osc_message(msg: OscMessage, prod: &Sender<AudioMessage>) {
    let addr = msg.addr.as_str();
    println!("Received OSC: {} | Args: {:?}", addr, msg.args);
    if addr.starts_with("/track/") {
        let parts: Vec<&str> = addr.split('/').collect();
        if parts.len() >= 4 {
            if let Ok(track_id) = parts[2].parse::<usize>() {
                let sub_addr = parts[3..].join("/");
                match sub_addr.as_str() {
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
                    // Two-Op specific (Track 0)
                    "legato" if track_id == 0 => {
                        if let Some(arg) = msg.args.get(0) {
                            let enabled = match arg {
                                rosc::OscType::Bool(b) => *b, rosc::OscType::Int(i) => *i > 0, rosc::OscType::Float(f) => *f > 0.5, _ => false,
                            };
                            let _ = prod.try_send(AudioMessage::TwoOpSetLegato { enabled });
                        }
                    }
                    "realtime" if track_id == 0 => {
                        if let Some(arg) = msg.args.get(0) {
                            let enabled = match arg {
                                rosc::OscType::Bool(b) => *b, rosc::OscType::Int(i) => *i > 0, rosc::OscType::Float(f) => *f > 0.5, _ => false,
                            };
                            let _ = prod.try_send(AudioMessage::TwoOpSetRealtimeUpdate { enabled });
                        }
                    }
                    "mode" if track_id == 0 => {
                        if let Some(arg) = msg.args.get(0) {
                            let mode_idx = arg.clone().int().unwrap_or(0);
                            let mode = match mode_idx {
                                0 => SynthMode::Additive, 1 => SynthMode::Am, 2 => SynthMode::Rm, 3 => SynthMode::Fm, _ => SynthMode::Additive,
                            };
                            let _ = prod.try_send(AudioMessage::TwoOpSetMode { mode });
                        }
                    }
                    "osc1/waveform" if track_id == 0 => {
                        if let Some(arg) = msg.args.get(0) {
                            let wf_idx = arg.clone().int().unwrap_or(0);
                            let waveform = match wf_idx {
                                0 => Waveform::Sine, 1 => Waveform::Saw, 2 => Waveform::Square, 3 => Waveform::Triangle, _ => Waveform::Sine,
                            };
                            let _ = prod.try_send(AudioMessage::TwoOpSetOsc1Waveform { waveform });
                        }
                    }
                    "osc2/waveform" if track_id == 0 => {
                        if let Some(arg) = msg.args.get(0) {
                            let wf_idx = arg.clone().int().unwrap_or(0);
                            let waveform = match wf_idx {
                                0 => Waveform::Sine, 1 => Waveform::Saw, 2 => Waveform::Square, 3 => Waveform::Triangle, _ => Waveform::Sine,
                            };
                            let _ = prod.try_send(AudioMessage::TwoOpSetOsc2Waveform { waveform });
                        }
                    }
                    "osc1/adsr" if track_id == 0 => {
                        if msg.args.len() >= 4 {
                            let a = msg.args[0].clone().float().unwrap_or(0.01);
                            let d = msg.args[1].clone().float().unwrap_or(0.1);
                            let s = msg.args[2].clone().float().unwrap_or(0.5);
                            let r = msg.args[3].clone().float().unwrap_or(0.5);
                            let _ = prod.try_send(AudioMessage::TwoOpSetOsc1Adsr { a, d, s, r });
                        }
                    }
                    "osc2/adsr" if track_id == 0 => {
                        if msg.args.len() >= 4 {
                            let a = msg.args[0].clone().float().unwrap_or(0.01);
                            let d = msg.args[1].clone().float().unwrap_or(0.1);
                            let s = msg.args[2].clone().float().unwrap_or(0.5);
                            let r = msg.args[3].clone().float().unwrap_or(0.5);
                            let _ = prod.try_send(AudioMessage::TwoOpSetOsc2Adsr { a, d, s, r });
                        }
                    }
                    "filter/adsr" if track_id == 0 => {
                        if msg.args.len() >= 4 {
                            let a = msg.args[0].clone().float().unwrap_or(0.01);
                            let d = msg.args[1].clone().float().unwrap_or(0.1);
                            let s = msg.args[2].clone().float().unwrap_or(0.5);
                            let r = msg.args[3].clone().float().unwrap_or(0.5);
                            let _ = prod.try_send(AudioMessage::TwoOpSetFilterAdsr { a, d, s, r });
                        }
                    }
                    "filter/params" if track_id == 0 => {
                        if msg.args.len() >= 3 {
                            let cutoff = msg.args[0].clone().float().unwrap_or(1000.0);
                            let q = msg.args[1].clone().float().unwrap_or(0.707);
                            let type_idx = msg.args[2].clone().int().unwrap_or(0);
                            let filter_type = match type_idx {
                                0 => FilterType::LowPass, 1 => FilterType::HighPass, 2 => FilterType::BandPass, 3 => FilterType::Notch, _ => FilterType::LowPass,
                            };
                            let _ = prod.try_send(AudioMessage::TwoOpSetFilterParams { cutoff, q, filter_type });
                        }
                    }
                    "filter/mod" if track_id == 0 => {
                        if msg.args.len() >= 2 {
                            let enabled = match msg.args[0] {
                                rosc::OscType::Bool(b) => b, rosc::OscType::Int(i) => i > 0, rosc::OscType::Float(f) => f > 0.5, _ => false,
                            };
                            let amount = msg.args[1].clone().float().unwrap_or(0.0);
                            let _ = prod.try_send(AudioMessage::TwoOpSetFilterMod { enabled, amount });
                        }
                    }
                    "mod/params" if track_id == 0 => {
                        if msg.args.len() >= 3 {
                            let index = msg.args[0].clone().float().unwrap_or(1.0);
                            let ratio = msg.args[1].clone().float().unwrap_or(1.0);
                            let detune = msg.args[2].clone().float().unwrap_or(0.0);
                            let _ = prod.try_send(AudioMessage::TwoOpSetModulationParams { index, ratio, detune });
                        }
                    }
                    // Sampler specific (Track 1)
                    "sample/load" if track_id == 1 => {
                        if let Some(path) = msg.args.get(0).and_then(|a| a.clone().string()) {
                            let _ = prod.try_send(AudioMessage::SamplerLoadSample { path });
                        }
                    }
                    "sample/mode" if track_id == 1 => {
                        if let Some(arg) = msg.args.get(0) {
                            let mode_idx = arg.clone().int().unwrap_or(0);
                            let mode = match mode_idx {
                                0 => PlayMode::OneShot,
                                1 => PlayMode::Loop,
                                _ => PlayMode::OneShot,
                            };
                            let _ = prod.try_send(AudioMessage::SamplerSetPlayMode { mode });
                        }
                    }
                    "sample/pitch" if track_id == 1 => {
                        if let Some(ratio) = msg.args.get(0).and_then(|a| a.clone().float()) {
                            let _ = prod.try_send(AudioMessage::SamplerSetPitchRatio { ratio });
                        }
                    }
                    "sample/speed" if track_id == 1 => {
                        if let Some(ratio) = msg.args.get(0).and_then(|a| a.clone().float()) {
                            let _ = prod.try_send(AudioMessage::SamplerSetSpeedRatio { ratio });
                        }
                    }
                    _ => {}
                }
            }
        }
    } else if addr == "/mixer/master/volume" {
        if let Some(vol) = msg.args.get(0).and_then(|a| a.clone().float()) {
            let _ = prod.try_send(AudioMessage::SetMasterVolume { volume: vol });
        }
    }
}

