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

    // Parse command line arguments
    let mut input_path: Option<String> = None;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--verbose" => {
                verbose = true;
            }
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
            eprintln!("Usage: {} [OPTIONS] <input.wav>", args[0]);
            eprintln!();
            eprintln!("Detects FT8 signals in a 15-second WAV file (12 kHz, mono).");
            eprintln!();
            eprintln!("Options:");
            eprintln!("  -v, --verbose    Enable verbose debug output");
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
        println!("Detected {} candidate signal(s) (coarse sync)", candidates.len());
        println!();

        // Apply fine sync to top 5 candidates
        let num_fine = 5.min(candidates.len());
        println!("Applying fine synchronization to top {} candidates...", num_fine);
        println!();

        let mut refined = Vec::new();
        for (i, cand) in candidates.iter().take(num_fine).enumerate() {
            print!("  {}. Refining {:7.1} Hz @ {:6.3} s ... ", i+1, cand.frequency, cand.time_offset);
            match sync::fine_sync(&signal_15s, cand) {
                Ok(refined_cand) => {
                    println!("OK: {:7.1} Hz @ {:6.3} s (sync={:.2e})",
                        refined_cand.frequency,
                        refined_cand.time_offset,
                        refined_cand.sync_power
                    );
                    refined.push(refined_cand);
                }
                Err(e) => {
                    println!("FAILED: {}", e);
                }
            }
        }

        println!();
        println!("Fine-sync results:");
        println!("  Freq (Hz)  Time (s)  Sync Power  dFreq    dTime");
        println!("  ---------  --------  ----------  ------  --------");
        for (i, refined_cand) in refined.iter().enumerate() {
            let coarse_cand = &candidates[i];
            let df = refined_cand.frequency - coarse_cand.frequency;
            let dt = refined_cand.time_offset - coarse_cand.time_offset;
            println!("  {:9.1}  {:8.3}  {:10.2e}  {:+6.2}  {:+8.3}",
                refined_cand.frequency,
                refined_cand.time_offset,
                refined_cand.sync_power,
                df,
                dt
            );
        }

        // Try to extract symbols and decode the best candidate
        println!();
        println!("Attempting symbol extraction and decoding...");
        println!();

        // Sort by sync power to get best candidate
        let mut best_candidates = refined.clone();
        best_candidates.sort_by(|a, b| b.sync_power.partial_cmp(&a.sync_power).unwrap_or(std::cmp::Ordering::Equal));

        for (i, cand) in best_candidates.iter().take(3).enumerate() {
            print!("  {}. Extracting {:7.1} Hz @ {:6.3} s ... ", i+1, cand.frequency, cand.time_offset);

            // Multi-pass decoding: try each scale with nsym=1,2,3 before moving to next scale
            // This allows nsym=2/3 to potentially help at lower scales
            let scaling_factors = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0];
            let nsym_values = [1, 2, 3];
            let mut decode_success = false;
            let mut total_passes = 0;

            // Pre-compute LLRs for all nsym values (like WSJT-X)
            use std::collections::HashMap;
            let mut llr_cache: HashMap<usize, Vec<f32>> = HashMap::new();
            for &nsym in &nsym_values {
                let mut llr = vec![0.0f32; 174];
                match sync::extract_symbols(&signal_15s, cand, nsym, &mut llr) {
                    Ok(_) => {
                        llr_cache.insert(nsym, llr);
                    }
                    Err(e) => {
                        if verbose {
                            eprintln!("     Warning: nsym={} extraction failed: {}", nsym, e);
                        }
                    }
                }
            }

            if llr_cache.is_empty() {
                println!("FAILED: All nsym extractions failed");
                continue;
            }

            println!("OK (LLRs computed for nsym={})",
                llr_cache.keys().map(|k| k.to_string()).collect::<Vec<_>>().join(","));

            // Try each scale with all nsym values
            'outer: for &scale in &scaling_factors {
                if decode_success {
                    break;
                }

                for &nsym in &nsym_values {
                    if decode_success {
                        break;
                    }

                    if let Some(base_llr) = llr_cache.get(&nsym) {
                        total_passes += 1;

                        // Scale LLRs
                        let mut scaled_llr = base_llr.clone();
                        for i in 0..174 {
                            scaled_llr[i] *= scale;
                        }

                        if total_passes == 1 {
                            print!("     Decoding with LDPC ({} scales × {} nsym)... ", scaling_factors.len(), nsym_values.len());
                        }

                        match rustyft8::ldpc::decode(&scaled_llr, 100) {
                            Some((decoded_bits, iterations)) => {
                                if total_passes == 1 {
                                    print!("SUCCESS (scale={:.1}, nsym={}, {} iters)", scale, nsym, iterations);
                                } else {
                                    println!();
                                    print!("     SUCCESS on pass {} (scale={:.1}, nsym={}, {} iters)", total_passes, scale, nsym, iterations);
                                }

                                // LDPC returns 91 bits (77 info + 14 CRC), we need only the first 77
                                use bitvec::prelude::*;
                                let info_bits: BitVec<u8, Msb0> = decoded_bits.iter().take(77).collect();

                                // Convert bits to message (no hash cache available)
                                match rustyft8::decode(&info_bits, None) {
                                    Ok(message) => {
                                        println!(" → \"{}\"", message);
                                        decode_success = true;
                                        break 'outer;
                                    }
                                    Err(e) => {
                                        println!(" → Failed to unpack: {}", e);
                                    }
                                }
                            }
                            None => {
                                // Continue to next nsym/scale
                            }
                        }
                    }
                }
            }

            if !decode_success {
                println!("FAILED (all {} passes)", total_passes);
            }
        }

        println!();
        println!("No signals successfully decoded.");
    }
}
