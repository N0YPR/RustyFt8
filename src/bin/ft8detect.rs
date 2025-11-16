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

            // TEMP: Also try with COARSE frequency (before fine_sync)
            let mut test_cand = cand.clone();
            if (test_cand.frequency - 1500.0).abs() < 1.0 {
                test_cand.frequency = 1500.0; // Force to 1500.0 Hz for testing
                eprintln!("    (Testing with forced 1500.0 Hz instead of {:.1} Hz)", cand.frequency);
            }

            let mut llr = vec![0.0f32; 174];
            match sync::extract_symbols(&signal_15s, &test_cand, &mut llr) {
                Ok(_) => {
                    println!("OK");

                    // Show LLR statistics
                    let mean_llr = llr.iter().sum::<f32>() / 174.0;
                    let mean_abs_llr = llr.iter().map(|x| x.abs()).sum::<f32>() / 174.0;
                    println!("     LLR stats: mean={:.2}, mean_abs={:.2}", mean_llr, mean_abs_llr);

                    // Try multiple decoding passes with different LLR scalings (like WSJT-X)
                    let scaling_factors = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0];
                    let mut decode_success = false;

                    for (pass, &scale) in scaling_factors.iter().enumerate() {
                        if decode_success {
                            break;
                        }

                        // Scale LLRs
                        let mut scaled_llr = llr.clone();
                        for i in 0..174 {
                            scaled_llr[i] *= scale;
                        }

                        if pass == 0 {
                            print!("     Decoding with LDPC (trying {} scaling factors)... ", scaling_factors.len());
                        }

                        match rustyft8::ldpc::decode(&scaled_llr, 100) {
                            Some((decoded_bits, iterations)) => {
                                if pass == 0 {
                                    print!("SUCCESS (pass {}, scale={:.1}, {} iters)", pass + 1, scale, iterations);
                                } else {
                                    println!();
                                    print!("     SUCCESS on pass {} (scale={:.1}, {} iters)", pass + 1, scale, iterations);
                                }

                                // LDPC returns 91 bits (77 info + 14 CRC), we need only the first 77
                                use bitvec::prelude::*;
                                let info_bits: BitVec<u8, Msb0> = decoded_bits.iter().take(77).collect();

                                // Convert bits to message (no hash cache available)
                                match rustyft8::decode(&info_bits, None) {
                                    Ok(message) => {
                                        println!(" → \"{}\"", message);
                                        decode_success = true;
                                    }
                                    Err(e) => {
                                        println!(" → Failed to unpack: {}", e);
                                    }
                                }
                            }
                            None => {
                                // Continue to next scaling factor
                            }
                        }
                    }

                    if !decode_success {
                        println!("FAILED (all {} passes)", scaling_factors.len());
                    }
                }
                Err(e) => {
                    println!("FAILED: {}", e);
                }
            }
        }

        println!();
        println!("No signals successfully decoded.");
    }
}
