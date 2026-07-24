//! Standalone performance CLI synthesizer for Aillen.
//!
//! Hosts the real-time audio thread and a UDP OSC server thread to process incoming events.

mod audio;
mod osc;
mod ui;

use clap::Parser;
use std::net::UdpSocket;
use rosc::{OscPacket, OscMessage, OscType};
use crossbeam_channel::{bounded, Sender};
use audio::{AudioMessage, start_audio_thread};
use ui::{UiHandle, start_ui_thread};

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

    let mut bank = aillen_core::sample_bank::SampleBank::new();
    let target_dir = args.samples_dir.or_else(|| {
        std::env::var("HOME")
            .map(|h| format!("{}/Desktop/KairosSamples", h))
            .ok()
    });

    if let Some(ref dir) = target_dir {
        if let Err(e) = bank.load_directory(dir) {
            eprintln!("SampleBank error (directory might not exist): {:?}", e);
        }
    }
    
    // 4 tracks: Track 0 (TwoOp), Track 1-3 (Sampler)
    let num_tracks = 6;
    let ui_handle = UiHandle::new(num_tracks);

    // Start UI thread
    start_ui_thread(ui_handle.clone(), num_tracks);

    // Create crossbeam channel for passing messages from OSC server to CPAL audio thread
    let (prod, cons) = bounded::<AudioMessage>(256);
    
    let _stream = start_audio_thread(cons, args.voices, args.device_index, ui_handle.clone())?;
    
    let socket = UdpSocket::bind(&format!("0.0.0.0:{}", args.port))?;
    
    let mut buf = [0u8; 2048];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, _client_addr)) => {
                let packet_slice = &buf[..size];
                match rosc::decoder::decode_udp(packet_slice) {
                    Ok((_, packet)) => handle_osc_packet(packet, &prod, &bank, &ui_handle),
                    Err(e) => eprintln!("Failed to decode OSC packet: {:?}", e),
                }
            }
            Err(e) => eprintln!("Socket receive error: {}", e),
        }
    }
}

use aillen_core::sample_bank::SampleBank;
use osc::{parse_track_command, parse_mixer_command};

/// Formats an OSC command cleanly without data type wrappers or track/mixer prefix.
fn format_clean_osc_msg(sub_path: &str, args: &[OscType]) -> String {
    if args.is_empty() {
        sub_path.to_string()
    } else {
        let formatted_args: Vec<String> = args.iter().map(|arg| match arg {
            OscType::Int(i) => i.to_string(),
            OscType::Float(f) => {
                let s = format!("{:.2}", f);
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            }
            OscType::String(s) => s.clone(),
            OscType::Bool(b) => b.to_string(),
            OscType::Long(l) => l.to_string(),
            OscType::Double(d) => {
                let s = format!("{:.2}", d);
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            }
            OscType::Char(c) => c.to_string(),
            _ => format!("{:?}", arg),
        }).collect();
        format!("{}: {}", sub_path, formatted_args.join(", "))
    }
}

fn handle_osc_packet(packet: OscPacket, prod: &Sender<AudioMessage>, bank: &SampleBank, ui_handle: &UiHandle) {
    match packet {
        OscPacket::Message(msg) => {
            route_and_record_msg(msg, prod, bank, ui_handle);
        }
        OscPacket::Bundle(bundle) => {
            for item in bundle.content {
                if let OscPacket::Message(m) = item {
                    route_and_record_msg(m, prod, bank, ui_handle);
                }
            }
        }
    }
}

/// Routes a single decoded OSC message to the audio message queue and updates UI text state cleanly.
fn route_and_record_msg(msg: OscMessage, prod: &Sender<AudioMessage>, bank: &SampleBank, ui_handle: &UiHandle) {
    let addr = msg.addr.as_str();
    if addr.starts_with("/track/") {
        let parts: Vec<&str> = addr.split('/').collect();
        if parts.len() >= 4 {
            if let Ok(track_id) = parts[2].parse::<usize>() {
                let sub_addr = parts[3..].join("/");
                
                // Check if this is a note off command
                let is_note_off = sub_addr == "note/off" 
                    || sub_addr == "note_off" 
                    || sub_addr == "off" 
                    || sub_addr.ends_with("/off")
                    || sub_addr == "note_off_all"
                    || sub_addr == "stop";
                if is_note_off {
                    ui_handle.clear_track_osc(track_id);
                } else {
                    let clean_str = format_clean_osc_msg(&sub_addr, &msg.args);
                    ui_handle.push_track_osc(track_id, clean_str);
                }

                parse_track_command(track_id, &sub_addr, &msg, prod, bank);
            }
        }
    } else if addr.starts_with("/mixer/") {
        let sub_path = &addr["/mixer/".len()..];
        let clean_str = format_clean_osc_msg(sub_path, &msg.args);
        ui_handle.push_master_osc(clean_str);
        parse_mixer_command(addr, &msg, prod);
    }
}
