//! FT8 Signal Detector and Decoder
//!
//! Reads a WAV file and decodes all FT8 signals present.
//!
//! **Usage**:
//! ```bash
//! cargo run --bin ft8detect -- input.wav
//! ```
//!
//! **Output**:
//! List of decoded messages with frequency, time, SNR, and message text.

use rustyft8::{decode_ft8, DecodedMessage, DecoderConfig};
use std::env;
use std::fs::File;
use std::io::Read;

/// Read WAV file and return samples
fn read_wav(path: &str) -> Result<Vec<f32>, String> {
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open file '{}': {}", path, e))?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Skip 44-byte WAV header
    if bytes.len() < 44 {
        return Err("File too small to be a valid WAV".to_string());
    }

    let data = &bytes[44..];

    // Convert 16-bit PCM to f32
    let num_samples = data.len() / 2;
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let idx = i * 2;
        if idx + 1 < data.len() {
            // Little-endian 16-bit signed integer
            let sample_i16 = i16::from_le_bytes([data[idx], data[idx + 1]]);
            // Convert to f32 in range [-1, 1]
            let sample_f32 = sample_i16 as f32 / 32768.0;
            samples.push(sample_f32);
        }
    }

    Ok(samples)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse command line arguments
    let mut input_path: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            arg if !arg.starts_with('-') => {
                input_path = Some(arg.to_string());
            }
            _ => {
                eprintln!("Unknown option: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let input_path = match input_path {
        Some(path) => path,
        None => {
            eprintln!("Usage: {} <input.wav>", args[0]);
            eprintln!();
            eprintln!("Decodes all FT8 signals in a 15-second WAV file (12 kHz, mono).");
            std::process::exit(1);
        }
    };

    println!("Reading WAV file: {}", input_path);

    // Read WAV file
    let signal = match read_wav(&input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading WAV: {}", e);
            std::process::exit(1);
        }
    };

    println!("  Samples: {}", signal.len());
    println!("  Duration: {:.2} seconds", signal.len() as f32 / 12000.0);

    // Pad or truncate to exactly 15 seconds
    const NMAX: usize = 15 * 12000;
    let mut signal_15s = signal;
    if signal_15s.len() < NMAX {
        println!("  Padding to 15 seconds...");
        signal_15s.resize(NMAX, 0.0);
    } else if signal_15s.len() > NMAX {
        println!("  Truncating to 15 seconds...");
        signal_15s.truncate(NMAX);
    }

    println!();
    println!("Decoding FT8 signals...");
    println!("  Frequency range: 100 - 3000 Hz");
    println!();

    // Use the multi-signal decoder with callback
    let config = DecoderConfig::default();
    let mut message_count = 0;

    match decode_ft8(&signal_15s, &config, |msg: DecodedMessage| {
        message_count += 1;
        println!("{:7.1} Hz  {:+5.2} s  {:3} dB  \"{}\"",
            msg.frequency,
            msg.time_offset,
            msg.snr_db,
            msg.message
        );
    }) {
        Ok(total) => {
            println!();
            if total == 0 {
                println!("No signals decoded.");
            } else {
                println!("Decoded {} signal(s).", total);
            }
        }
        Err(e) => {
            eprintln!("Decode failed: {}", e);
            std::process::exit(1);
        }
    }
}
