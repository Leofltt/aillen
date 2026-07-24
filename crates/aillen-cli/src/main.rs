//! Standalone performance CLI synthesizer for Aillen.
//!
//! Hosts the real-time audio thread and a UDP OSC server thread to process incoming events.

mod audio;
mod osc;

use clap::Parser;
use std::net::UdpSocket;
use rosc::{OscPacket, OscMessage};
use crossbeam_channel::{bounded, Sender};
use audio::{AudioMessage, start_audio_thread};

/// Command line arguments parsed by Clap.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// UDP port to listen for incoming OSC packets.
    #[arg(short, long, default_value_t = 8000)]
    port: u16,
    /// Number of polyphonic synth voices to allocate.
    #[arg(short, long, default_value_t = 8)]
    voices: usize,
    /// Optional index of the host audio output device to use.
    #[arg(short, long)]
    device_index: Option<usize>,
    /// Flags to list all available host output devices and exit.
    #[arg(short, long)]
    list_devices: bool,
    /// Path to a directory containing audio samples to load on startup.
    #[arg(short, long)]
    samples_dir: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.list_devices {
        audio::list_audio_devices()?;
        return Ok(());
    }
    println!("Aillen Synthesizer CLI Starting up...");

    let mut bank = aillen_core::sample_bank::SampleBank::new();
    let target_dir = args.samples_dir.or_else(|| {
        std::env::var("HOME")
            .map(|h| format!("{}/Desktop/KairosSamples", h))
            .ok()
    });

    if let Some(ref dir) = target_dir {
        println!("SampleBank: Scanning directory: {}", dir);
        if let Err(e) = bank.load_directory(dir) {
            eprintln!("SampleBank error (directory might not exist): {:?}", e);
        } else {
            println!("SampleBank: Loaded {} samples.", bank.samples.len());
        }
    }
    
    // Create crossbeam channel for passing messages from OSC server to CPAL audio thread
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
                    Ok((_, OscPacket::Message(msg))) => handle_osc_message(msg, &prod, &bank),
                    Ok((_, OscPacket::Bundle(bundle))) => {
                        for packet in bundle.content {
                            if let OscPacket::Message(msg) = packet {
                                handle_osc_message(msg, &prod, &bank);
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

use aillen_core::sample_bank::SampleBank;
use osc::{parse_track_command, parse_mixer_command};

/// Routes a single decoded OSC message to the audio message queue.
fn handle_osc_message(msg: OscMessage, prod: &Sender<AudioMessage>, bank: &SampleBank) {
    let addr = msg.addr.as_str();
    println!("Received OSC: {} | Args: {:?}", addr, msg.args);
    if addr.starts_with("/track/") {
        let parts: Vec<&str> = addr.split('/').collect();
        if parts.len() >= 4 {
            if let Ok(track_id) = parts[2].parse::<usize>() {
                let sub_addr = parts[3..].join("/");
                parse_track_command(track_id, &sub_addr, &msg, prod, bank);
            }
        }
    } else if addr.starts_with("/mixer/") {
        parse_mixer_command(addr, &msg, prod);
    }
}

