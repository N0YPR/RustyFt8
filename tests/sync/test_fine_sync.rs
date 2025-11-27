//! Fine sync validation test - compare with WSJT-X ft8b.f90 reference

use rustyft8::sync::{coarse_sync, fine_sync};
use std::collections::HashMap;

#[path = "../test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn test_fine_sync_matches_wsjtx() {
    // Load test WAV file
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    // Run coarse sync to get candidates (same as reference data)
    let coarse_candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("Loaded {} coarse candidates", coarse_candidates.len());

    // Load WSJT-X fine sync reference data
    let wsjtx_ref = load_fine_sync_reference("tests/sync/fine_sync_ref.csv")
        .expect("Failed to load reference");

    println!("Loaded {} WSJT-X fine sync references", wsjtx_ref.len());

    // Compare our fine sync with WSJT-X
    let mut freq_diffs = Vec::new();
    let mut time_diffs = Vec::new();
    let mut exact_freq_matches = 0;
    let mut exact_time_matches = 0;

    for (idx, coarse_cand) in coarse_candidates.iter().enumerate() {
        // Run our fine sync
        let fine_cand = match fine_sync(&signal, coarse_cand) {
            Ok(c) => c,
            Err(e) => {
                println!("  [{}] Fine sync failed: {}", idx, e);
                continue;
            }
        };

        // Get WSJT-X reference for this candidate
        let wsjtx = match wsjtx_ref.get(&idx) {
            Some(w) => w,
            None => {
                println!("  [{}] No WSJT-X reference found", idx);
                continue;
            }
        };

        // Calculate differences
        let freq_diff = (fine_cand.frequency - wsjtx.freq_out).abs();
        let time_diff = (fine_cand.time_offset - wsjtx.time_out).abs();

        freq_diffs.push(freq_diff);
        time_diffs.push(time_diff);

        if freq_diff < 0.1 {
            exact_freq_matches += 1;
        }
        if time_diff < 0.005 {
            exact_time_matches += 1;
        }

        // Log significant differences
        if freq_diff > 5.0 || time_diff > 0.1 {
            println!(
                "  [{}] Large diff: freq_in={:.1}, RustyFt8_freq={:.1}, WSJTX_freq={:.1} (Δ={:.1} Hz), \
                 time_in={:.3}, RustyFt8_time={:.3}, WSJTX_time={:.3} (Δ={:.3} s)",
                idx,
                coarse_cand.frequency,
                fine_cand.frequency,
                wsjtx.freq_out,
                freq_diff,
                coarse_cand.time_offset,
                fine_cand.time_offset,
                wsjtx.time_out,
                time_diff
            );
        }
    }

    // Calculate statistics
    let n = freq_diffs.len();
    println!("\n=== Fine Sync Validation Results ===");
    println!("Compared {} candidates", n);

    let freq_mean = freq_diffs.iter().sum::<f32>() / n as f32;
    let time_mean = time_diffs.iter().sum::<f32>() / n as f32;

    let mut freq_sorted = freq_diffs.clone();
    freq_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let freq_median = freq_sorted[n / 2];
    let freq_p95 = freq_sorted[(n as f32 * 0.95) as usize];

    let mut time_sorted = time_diffs.clone();
    time_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let time_median = time_sorted[n / 2];
    let time_p95 = time_sorted[(n as f32 * 0.95) as usize];

    println!("\nFrequency differences:");
    println!("  Mean: {:.3} Hz", freq_mean);
    println!("  Median: {:.3} Hz", freq_median);
    println!("  95th percentile: {:.3} Hz", freq_p95);
    println!("  Exact matches (<0.1 Hz): {}/{} ({:.1}%)",
             exact_freq_matches, n, 100.0 * exact_freq_matches as f32 / n as f32);

    println!("\nTime differences:");
    println!("  Mean: {:.3} s", time_mean);
    println!("  Median: {:.3} s", time_median);
    println!("  95th percentile: {:.3} s", time_p95);
    println!("  Exact matches (<5 ms): {}/{} ({:.1}%)",
             exact_time_matches, n, 100.0 * exact_time_matches as f32 / n as f32);

    // Success criteria: 95% of candidates should be within reasonable tolerances
    let freq_good = freq_diffs.iter().filter(|&&d| d < 1.0).count();
    let time_good = time_diffs.iter().filter(|&&d| d < 0.05).count();

    let freq_pct = 100.0 * freq_good as f32 / n as f32;
    let time_pct = 100.0 * time_good as f32 / n as f32;

    println!("\n=== Pass/Fail Criteria ===");
    println!("Frequency within 1.0 Hz: {:.1}% (target: >95%)", freq_pct);
    println!("Time within 50 ms: {:.1}% (target: >95%)", time_pct);

    assert!(
        freq_pct > 95.0,
        "Frequency match rate {:.1}% is below 95% target",
        freq_pct
    );
    assert!(
        time_pct > 95.0,
        "Time match rate {:.1}% is below 95% target",
        time_pct
    );
}

#[derive(Debug)]
struct FineSyncReference {
    freq_in: f32,
    time_in: f32,
    sync_in: f32,
    freq_out: f32,
    time_out: f32,
    sync_out: f32,
    nharderrors: i32,
    nbadcrc: i32,
}

fn load_fine_sync_reference(path: &str) -> Result<HashMap<usize, FineSyncReference>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let mut references = HashMap::new();
    let mut idx = 0;

    for (line_num, line) in content.lines().enumerate() {
        // Skip header lines
        if line_num < 2 {
            continue;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse CSV: freq_in,time_in,sync_in,freq_out,time_out,sync_out,nharderrors,nbadcrc
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 8 {
            return Err(format!("Invalid CSV line {}: {}", line_num, line));
        }

        let reference = FineSyncReference {
            freq_in: parts[0].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
            time_in: parts[1].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
            sync_in: parts[2].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
            freq_out: parts[3].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
            time_out: parts[4].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
            sync_out: parts[5].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
            nharderrors: parts[6].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
            nbadcrc: parts[7].trim().parse().map_err(|e| format!("Parse error: {}", e))?,
        };

        references.insert(idx, reference);
        idx += 1;
    }

    Ok(references)
}
