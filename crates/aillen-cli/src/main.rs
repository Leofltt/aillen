mod audio;

use clap::Parser;
use std::net::UdpSocket;
use rosc::{OscPacket, OscMessage};
use crossbeam_channel::{bounded, Sender};
use audio::{AudioMessage, TwoOpMessage, start_audio_thread};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port for the OSC UDP server
    #[arg(short, long, default_value_t = 8000)]
    port: u16,

    /// Number of polyphonic voices (set to 1 for monosynth behavior)
    #[arg(short, long, default_value_t = 8)]
    voices: usize,

    /// Audio device index to use (see --list-devices)
    #[arg(short, long)]
    device_index: Option<usize>,

    /// List available audio devices and exit
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
    
    // Create a bounded channel for lock-free sending to the audio thread
    let (prod, cons) = bounded::<AudioMessage>(256);
    
    // Start the audio stream and keep it alive
    let _stream = start_audio_thread(cons, args.voices, args.device_index)?;
    println!("Audio thread created successfully.");
    
    // Set up OSC UDP listener
    let socket = UdpSocket::bind(&format!("0.0.0.0:{}", args.port))?;
    println!("Listening for OSC messages on UDP port {}", args.port);
    
    let mut buf = [0u8; 2048];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _client_addr)) => {
                let packet_slice = &buf[..size];
                match rosc::decoder::decode_udp(packet_slice) {
                    Ok((_, OscPacket::Message(msg))) => {
                        handle_osc_message(msg, &prod);
                    }
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

fn handle_osc_message(msg: OscMessage, prod: &Sender<AudioMessage>) {
    let addr = msg.addr.as_str();
    println!("Received OSC: {} | Args: {:?}", addr, msg.args);
    if addr.starts_with("/two_op/") {
        let sub_addr = &addr[8..];
        match sub_addr {
            "note/on" => {
                if let Some(freq) = msg.args.get(0).and_then(|a| a.clone().float()) {
                    let vel = msg.args.get(1).and_then(|a| a.clone().float()).unwrap_or(1.0);
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::NoteOn { freq, vel }));
                }
            }
            "note/off" => {
                if let Some(freq) = msg.args.get(0).and_then(|a| a.clone().float()) {
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::NoteOff { freq }));
                } else {
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::NoteOffAll));
                }
            }
            "note" => {
                if msg.args.len() >= 2 {
                    let freq = msg.args[0].clone().float().unwrap_or(440.0);
                    let duration_ms = msg.args[1].clone().float().unwrap_or(100.0);
                    let vel = msg.args.get(2).and_then(|a| a.clone().float()).unwrap_or(1.0);
                    println!("Triggering timed note: {}Hz for {}ms", freq, duration_ms);
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::TimedNote { freq, vel, duration_ms }));
                } else {
                    println!("Timed note message missing arguments!");
                }
            }
            "legato" => {
                if let Some(arg) = msg.args.get(0) {
                    let enabled = match arg {
                        rosc::OscType::Bool(b) => *b,
                        rosc::OscType::Int(i) => *i > 0,
                        rosc::OscType::Float(f) => *f > 0.5,
                        _ => false,
                    };
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetLegato { enabled }));
                }
            }
            "mode" => {
                if let Some(arg) = msg.args.get(0) {
                    let mode_idx = arg.clone().int().unwrap_or(0);
                    let mode = match mode_idx {
                        0 => SynthMode::Additive,
                        1 => SynthMode::Am,
                        2 => SynthMode::Rm,
                        3 => SynthMode::Fm,
                        _ => SynthMode::Additive,
                    };
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetMode { mode }));
                }
            }
            "osc1/waveform" => {
                if let Some(arg) = msg.args.get(0) {
                    let wf_idx = arg.clone().int().unwrap_or(0);
                    let waveform = match wf_idx {
                        0 => Waveform::Sine, 1 => Waveform::Saw, 2 => Waveform::Square, 3 => Waveform::Triangle, _ => Waveform::Sine,
                    };
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetOsc1Waveform { waveform }));
                }
            }
            "osc2/waveform" => {
                if let Some(arg) = msg.args.get(0) {
                    let wf_idx = arg.clone().int().unwrap_or(0);
                    let waveform = match wf_idx {
                        0 => Waveform::Sine, 1 => Waveform::Saw, 2 => Waveform::Square, 3 => Waveform::Triangle, _ => Waveform::Sine,
                    };
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetOsc2Waveform { waveform }));
                }
            }
            "osc1/adsr" => {
                if msg.args.len() >= 4 {
                    let a = msg.args[0].clone().float().unwrap_or(0.01);
                    let d = msg.args[1].clone().float().unwrap_or(0.1);
                    let s = msg.args[2].clone().float().unwrap_or(0.5);
                    let r = msg.args[3].clone().float().unwrap_or(0.5);
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetOsc1Adsr { a, d, s, r }));
                }
            }
            "osc2/adsr" => {
                if msg.args.len() >= 4 {
                    let a = msg.args[0].clone().float().unwrap_or(0.01);
                    let d = msg.args[1].clone().float().unwrap_or(0.1);
                    let s = msg.args[2].clone().float().unwrap_or(0.5);
                    let r = msg.args[3].clone().float().unwrap_or(0.5);
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetOsc2Adsr { a, d, s, r }));
                }
            }
            "filter/adsr" => {
                if msg.args.len() >= 4 {
                    let a = msg.args[0].clone().float().unwrap_or(0.01);
                    let d = msg.args[1].clone().float().unwrap_or(0.1);
                    let s = msg.args[2].clone().float().unwrap_or(0.5);
                    let r = msg.args[3].clone().float().unwrap_or(0.5);
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetFilterAdsr { a, d, s, r }));
                }
            }
            "filter/params" => {
                if msg.args.len() >= 3 {
                    let cutoff = msg.args[0].clone().float().unwrap_or(1000.0);
                    let q = msg.args[1].clone().float().unwrap_or(0.707);
                    let type_idx = msg.args[2].clone().int().unwrap_or(0);
                    let filter_type = match type_idx {
                        0 => FilterType::LowPass, 1 => FilterType::HighPass, 2 => FilterType::BandPass, 3 => FilterType::Notch, _ => FilterType::LowPass,
                    };
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetFilterParams { cutoff, q, filter_type }));
                }
            }
            "filter/mod" => {
                if msg.args.len() >= 2 {
                    let enabled = match msg.args[0] {
                        rosc::OscType::Bool(b) => b, rosc::OscType::Int(i) => i > 0, rosc::OscType::Float(f) => f > 0.5, _ => false,
                    };
                    let amount = msg.args[1].clone().float().unwrap_or(0.0);
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetFilterMod { enabled, amount }));
                }
            }
            "mod/params" => {
                if msg.args.len() >= 3 {
                    let index = msg.args[0].clone().float().unwrap_or(1.0);
                    let ratio = msg.args[1].clone().float().unwrap_or(1.0);
                    let detune = msg.args[2].clone().float().unwrap_or(0.0);
                    let _ = prod.try_send(AudioMessage::TwoOp(TwoOpMessage::SetModulationParams { index, ratio, detune }));
                }
            }
            _ => {}
        }
    }
}
