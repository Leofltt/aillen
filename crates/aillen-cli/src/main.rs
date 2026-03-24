mod audio;

use clap::Parser;
use std::net::UdpSocket;
use rosc::{OscPacket, OscMessage};
use crossbeam_channel::{bounded, Sender};
use audio::{AudioMessage, start_audio_thread};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port for the OSC UDP server
    #[arg(short, long, default_value_t = 8000)]
    port: u16,

    /// Number of polyphonic voices (set to 1 for monosynth behavior)
    #[arg(short, long, default_value_t = 8)]
    voices: usize,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    
    println!("Aillen Synthesizer CLI Starting up...");
    
    // Create a bounded channel for lock-free sending to the audio thread
    let (prod, cons) = bounded::<AudioMessage>(256);
    
    // Start the audio stream and keep it alive
    let _stream = start_audio_thread(cons, args.voices)?;
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

fn handle_osc_message(msg: OscMessage, prod: &Sender<AudioMessage>) {
    match msg.addr.as_str() {
        "/note/on" => {
            let args = msg.args;
            if args.len() >= 1 {
                let freq = args[0].clone().float().unwrap_or(440.0);
                let vel = if args.len() > 1 {
                    args[1].clone().float().unwrap_or(1.0)
                } else {
                    1.0
                };
                let _ = prod.try_send(AudioMessage::NoteOn { freq, vel });
            }
        }
        "/note/off" => {
            let args = msg.args;
            if args.len() >= 1 {
                let freq = args[0].clone().float().unwrap_or(440.0);
                let _ = prod.try_send(AudioMessage::NoteOff { freq });
            } else {
                let _ = prod.try_send(AudioMessage::NoteOffAll);
            }
        }
        "/note" => {
            let args = msg.args;
            if args.len() >= 2 {
                let freq = args[0].clone().float().unwrap_or(440.0);
                let duration_ms = args[1].clone().float().unwrap_or(100.0);
                let vel = if args.len() > 2 {
                    args[2].clone().float().unwrap_or(1.0)
                } else {
                    1.0
                };
                let _ = prod.try_send(AudioMessage::TimedNote { freq, vel, duration_ms });
            }
        }
        "/legato" => {
            let args = msg.args;
            if args.len() >= 1 {
                let enabled = match args[0] {
                    rosc::OscType::Bool(b) => b,
                    rosc::OscType::Int(i) => i > 0,
                    rosc::OscType::Float(f) => f > 0.5,
                    _ => false,
                };
                let _ = prod.try_send(AudioMessage::SetLegato { enabled });
            }
        }
        _ => {
            // Unhandled OSC message
        }
    }
}
