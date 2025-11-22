//! Debug tool to analyze candidate detection
//!
//! Shows all candidates found during coarse sync to understand why signals are missed.

use rustyft8::sync;
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

    println!("=== CANDIDATE DETECTION ANALYSIS ===\n");

    // Run with lower threshold to see more candidates
    let sync_threshold = 0.4;
    let freq_min = 100.0;
    let freq_max = 3000.0;
    let max_candidates = 150;

    println!("Config:");
    println!("  freq_min: {} Hz", freq_min);
    println!("  freq_max: {} Hz", freq_max);
    println!("  sync_threshold: {}", sync_threshold);
    println!("  max_candidates: {}", max_candidates);
    println!();

    let candidates = sync::coarse_sync(
        &signal,
        freq_min,
        freq_max,
        sync_threshold,
        max_candidates,
    ).expect("Coarse sync failed");

    println!("Found {} candidates\n", candidates.len());

    // Sort by sync power (descending)
    let mut sorted_candidates = candidates.clone();
    sorted_candidates.sort_by(|a, b| b.sync_power.partial_cmp(&a.sync_power).unwrap_or(std::cmp::Ordering::Equal));

    // WSJT-X frequencies for comparison
    let wsjtx_signals = vec![
        (2571.0, 16, "W1FC F5BZB -08"),
        (2157.0, 12, "WM3PEN EA6VQ -09"),
        (1197.0, -2, "CQ F5RXL IN94"),
        (641.0, -12, "N1JFU EA6EE R-07"),
        (723.0, -7, "A92EE F5PSR -14"),
        (2695.0, -3, "K1BZM EA3GP -09"),
        (400.0, -16, "W0RSJ EA3BMU RR73"),
        (590.0, -14, "K1JT HA0DU KN07"),
        (2733.0, -7, "W1DIG SV9CVY -14"),
        (1648.0, -16, "K1JT EA3AGB -15"),
        (2852.0, -11, "XE2X HA2NP RR73"),
        (2522.0, -7, "K1BZM EA3CJ JN01"),
        (2546.0, -9, "WA2FZW DL5AXX RR73"),
        (2238.0, -14, "N1API HA6FQ -23"),
        (466.0, -2, "N1PJT HB9CQK -10"),
        (1513.0, -17, "N1API F2VX 73"),
        (2606.0, -17, "CQ DX DL8YHR JO41"),
        (2039.0, -20, "K1JT HA5WA 73"),
        (472.0, -6, "KD2UGC F6GCP R-23"),
        (2280.0, -17, "CQ EA2BFM IN83"),
        (244.0, -20, "K1BZM DK8NE -10"),
        (3390.0, -24, "TU; 7N9RST EI8TRF 589 5732"),
    ];

    println!("=== WSJT-X SIGNALS vs CANDIDATES ===\n");
    for (freq, snr, msg) in &wsjtx_signals {
        // Find closest candidate within ±10 Hz
        let closest = sorted_candidates.iter()
            .filter(|c| (c.frequency - freq).abs() < 10.0)
            .min_by(|a, b| {
                let dist_a = (a.frequency - freq).abs();
                let dist_b = (b.frequency - freq).abs();
                dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
            });

        if let Some(cand) = closest {
            println!("✓ {:4.0} Hz (SNR: {:3} dB) {}", freq, snr, msg);
            println!("  → Candidate @ {:.1} Hz, sync={:.3}, dt={:.2}s",
                     cand.frequency, cand.sync_power, cand.time_offset);
        } else {
            println!("✗ {:4.0} Hz (SNR: {:3} dB) {} - NO CANDIDATE FOUND", freq, snr, msg);
        }
    }

    println!("\n=== TOP 30 CANDIDATES BY SYNC POWER ===\n");
    for (idx, cand) in sorted_candidates.iter().take(30).enumerate() {
        println!("{:3}. {:.1} Hz  sync={:.3}  dt={:.2}s  noise={:.2e}",
                 idx + 1, cand.frequency, cand.sync_power, cand.time_offset, cand.baseline_noise);
    }
}
