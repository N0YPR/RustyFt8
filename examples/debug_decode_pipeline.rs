//! Debug tool to trace the decode pipeline
//!
//! Shows which candidates are selected for decoding and why some fail.

use rustyft8::{sync, DecoderConfig};
use hound;

fn read_wav_file(path: &str) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV file: {}", e))?;

    let spec = reader.spec();
    if spec.sample_rate != 12000 {
        return Err(format!("Expected 12000 Hz sample rate, got {}", spec.sample_rate));
    }

    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => {
                    reader.into_samples::<i16>()
                        .map(|s| s.map(|v| v as f32 / 32768.0))
                        .collect()
                }
                _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample)),
            }
        }
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>().collect()
        }
    };

    samples.map_err(|e| format!("Failed to read samples: {}", e))
}

fn main() {
    let wav_path = "tests/test_data/210703_133430.wav";
    let mut signal = read_wav_file(wav_path).expect("Failed to read WAV file");

    // Pad to 15 seconds
    let expected_samples = 15 * 12000;
    if signal.len() < expected_samples {
        signal.resize(expected_samples, 0.0);
    } else if signal.len() > expected_samples {
        signal.truncate(expected_samples);
    }

    let config = DecoderConfig {
        decode_top_n: 150,
        sync_threshold: 0.4,
        max_candidates: 150,
        ..DecoderConfig::default()
    };

    println!("=== DECODE PIPELINE ANALYSIS ===\n");
    println!("Config:");
    println!("  decode_top_n: {}", config.decode_top_n);
    println!("  sync_threshold: {}", config.sync_threshold);
    println!("  max_candidates: {}", config.max_candidates);
    println!();

    // Run coarse sync
    let candidates = sync::coarse_sync(
        &signal,
        config.freq_min,
        config.freq_max,
        config.sync_threshold,
        config.max_candidates,
    ).expect("Coarse sync failed");

    println!("Found {} candidates after coarse sync\n", candidates.len());

    // WSJT-X expected signals
    let expected = vec![
        (2571.0, "W1FC F5BZB -08"),
        (2157.0, "WM3PEN EA6VQ -09"),
        (1197.0, "CQ F5RXL IN94"),
        (641.0, "N1JFU EA6EE R-07"),
        (723.0, "A92EE F5PSR -14"),
        (2695.0, "K1BZM EA3GP -09"),
        (400.0, "W0RSJ EA3BMU RR73"),
        (590.0, "K1JT HA0DU KN07"),
        (2733.0, "W1DIG SV9CVY -14"),
        (1648.0, "K1JT EA3AGB -15"),
        (2852.0, "XE2X HA2NP RR73"),
        (2522.0, "K1BZM EA3CJ JN01"),
        (2546.0, "WA2FZW DL5AXX RR73"),
        (2238.0, "N1API HA6FQ -23"),
        (466.0, "N1PJT HB9CQK -10"),
        (1513.0, "N1API F2VX 73"),
        (2606.0, "CQ DX DL8YHR JO41"),
        (472.0, "KD2UGC F6GCP R-23"),
        (2280.0, "CQ EA2BFM IN83"),
    ];

    println!("=== EXPECTED SIGNALS IN DECODE LIST ===\n");
    for (idx, cand) in candidates.iter().take(config.decode_top_n).enumerate() {
        // Check if this matches an expected signal
        for (exp_freq, exp_msg) in &expected {
            if (cand.frequency - exp_freq).abs() < 10.0 {
                let in_decode = if idx < config.decode_top_n { "✓" } else { "✗" };
                println!("{} Rank {:3}: {:.1} Hz  sync={:.3}  dt={:.2}s  \"{}\"",
                         in_decode, idx + 1, cand.frequency, cand.sync_power, cand.time_offset, exp_msg);
            }
        }
    }

    println!("\n=== EXPECTED SIGNALS NOT IN DECODE LIST ===\n");
    for (exp_freq, exp_msg) in &expected {
        let found_in_top = candidates.iter().take(config.decode_top_n).any(|c| {
            (c.frequency - exp_freq).abs() < 10.0
        });

        if !found_in_top {
            // Find it in the full candidate list
            let found_anywhere = candidates.iter().enumerate().find(|(_, c)| {
                (c.frequency - exp_freq).abs() < 10.0
            });

            if let Some((idx, cand)) = found_anywhere {
                println!("✗ Rank {}: {:.1} Hz  sync={:.3}  dt={:.2}s  \"{}\" (rank too low!)",
                         idx + 1, cand.frequency, cand.sync_power, cand.time_offset, exp_msg);
            } else {
                println!("✗ NO CANDIDATE for {} Hz  \"{}\"", exp_freq, exp_msg);
            }
        }
    }

    println!("\n=== TOP 50 CANDIDATES BY SYNC POWER ===\n");
    for (idx, cand) in candidates.iter().take(50).enumerate() {
        // Mark if expected
        let is_expected = expected.iter().any(|(f, _)| (cand.frequency - f).abs() < 10.0);
        let marker = if is_expected { "★" } else { " " };
        println!("{} {:3}. {:.1} Hz  sync={:.3}  dt={:.2}s",
                 marker, idx + 1, cand.frequency, cand.sync_power, cand.time_offset);
    }
}
