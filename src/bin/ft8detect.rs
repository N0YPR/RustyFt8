//! FT8 Signal Detector
//!
//! Reads a WAV file and detects FT8 signals using Costas array correlation.
//!
//! **Usage**:
//! ```bash
//! cargo run --bin ft8detect -- input.wav
//! ```
//!
//! **Output**:
//! List of detected signals with frequency, time offset, and sync quality.

use rustyft8::sync;
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

    if args.len() != 2 {
        eprintln!("Usage: {} <input.wav>", args[0]);
        eprintln!();
        eprintln!("Detects FT8 signals in a 15-second WAV file (12 kHz, mono).");
        std::process::exit(1);
    }

    let input_path = &args[1];

    println!("Reading WAV file: {}", input_path);

    // Read WAV file
    let signal = match read_wav(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading WAV: {}", e);
            std::process::exit(1);
        }
    };

    println!("  Samples: {}", signal.len());
    println!("  Duration: {:.2} seconds", signal.len() as f32 / 12000.0);

    // Pad or truncate to exactly 15 seconds
    let mut signal_15s = signal;
    if signal_15s.len() < sync::NMAX {
        println!("  Padding to 15 seconds...");
        signal_15s.resize(sync::NMAX, 0.0);
    } else if signal_15s.len() > sync::NMAX {
        println!("  Truncating to 15 seconds...");
        signal_15s.truncate(sync::NMAX);
    }

    println!();
    println!("Detecting FT8 signals...");
    println!("  Frequency range: 100 - 3000 Hz");
    println!("  Sync threshold: 0.5 (lowered for testing)");
    println!();

    // Perform coarse synchronization with lower threshold for testing
    let candidates = match sync::coarse_sync(&signal_15s, 100.0, 3000.0, 0.5, 100) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Sync failed: {}", e);
            std::process::exit(1);
        }
    };

    if candidates.is_empty() {
        println!("No signals detected.");
    } else {
        println!("Detected {} candidate signal(s):", candidates.len());
        println!();
        println!("  Freq (Hz)  Time (s)  Sync Power");
        println!("  ---------  --------  ----------");
        for cand in &candidates {
            println!("  {:9.1}  {:8.3}  {:10.2}",
                cand.frequency,
                cand.time_offset,
                cand.sync_power
            );
        }
    }
}
