import time
import argparse
from pythonosc import udp_client

def test_aillen(port, sample_path):
    client = udp_client.SimpleUDPClient("127.0.0.1", port)
    
    print(f"Connecting to Aillen on port {port}...")

    # Set master volume and individual track gains/pans
    client.send_message("/mixer/master/volume", 0.8)
    
    client.send_message("/track/0/volume", 0.6)
    client.send_message("/track/0/pan", -0.4) # Synth slightly left
    client.send_message("/track/0/mute", False)

    client.send_message("/track/1/volume", 0.7)
    client.send_message("/track/1/pan", 0.4) # Sampler slightly right
    client.send_message("/track/1/mute", False)

    # Configure synth (Track 0) parameters
    client.send_message("/track/0/mode", 3) # FM mode
    client.send_message("/track/0/osc1/waveform", 1) # Sawtooth
    client.send_message("/track/0/osc1/adsr", [0.02, 0.15, 0.6, 0.2])

    if sample_path:
        print(f"\nLoading sample file into Track 1: {sample_path}")
        client.send_message("/track/1/sample/load", sample_path)
        time.sleep(1.0) # Wait for file loading to complete
        
        print("Starting Loop playback on Track 1...")
        client.send_message("/track/1/sample/mode", 1) # Loop mode
        client.send_message("/track/1/note/on", [261.63, 0.9]) # Trigger at root pitch (C4)
        time.sleep(1.0)

    # Play a melody on Track 0 (TwoOp Synth) concurrently!
    print("\nPlaying synth melody on Track 0 concurrently...")
    melody = [
        (220.0, 0.4), # A3
        (261.63, 0.4), # C4
        (293.66, 0.4), # D4
        (329.63, 0.4), # E4
        (440.0, 0.8), # A4
    ]

    for freq, duration in melody:
        print(f"Playing synth note: {freq} Hz")
        client.send_message("/track/0/note/on", [freq, 0.8])
        time.sleep(duration - 0.05)
        client.send_message("/track/0/note/off", freq)
        time.sleep(0.05)

    if sample_path:
        # Stop the sampler loop after melody finishes
        print("\nStopping sampler loop on Track 1...")
        client.send_message("/track/1/note/off", 261.63)
        time.sleep(0.5)

    print("\nTests complete!")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Test Aillen Synth & Sampler via OSC")
    parser.add_argument("--port", type=int, default=8000, help="OSC UDP Port")
    parser.add_argument("--sample", type=str, default="", help="Path to an audio file (wav, mp3, flac) to test the sampler")
    args = parser.parse_args()
    
    test_aillen(args.port, args.sample)
