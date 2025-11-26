//! Test compute_sync2d against WSJT-X reference implementation
//!
//! This test verifies that our sync2d correlation computation matches WSJT-X exactly.

use rustyft8::sync::{compute_spectra, compute_sync2d, NH1, NHSYM, MAX_LAG};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::collections::HashMap;

#[path = "../test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length, init_test_tracing};

/// Read sync2d CSV file (bin,freq,lag-62,...,lag62)
fn read_sync2d_csv(path: &str) -> Result<HashMap<usize, Vec<f32>>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open {}: {}", path, e))?;
    let reader = BufReader::new(file);

    let mut sync2d_map = HashMap::new();
    let mut lines = reader.lines();

    // Skip header line
    lines.next();

    for line in lines {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        let parts: Vec<&str> = line.split(',').collect();

        if parts.len() < 3 {
            continue;  // Skip invalid lines
        }

        let bin: usize = parts[0].trim().parse()
            .map_err(|e| format!("Failed to parse bin: {}", e))?;

        // Parse lag values (skip bin and freq columns)
        let mut lag_values = Vec::new();
        for i in 2..parts.len() {
            let val: f32 = parts[i].trim().parse()
                .map_err(|e| format!("Failed to parse value at col {}: {}", i, e))?;
            lag_values.push(val);
        }

        // Should have 2*MAX_LAG+1 = 125 lag values
        if lag_values.len() != (2 * MAX_LAG + 1) as usize {
            return Err(format!("Expected {} lag values, got {}", 2 * MAX_LAG + 1, lag_values.len()));
        }

        sync2d_map.insert(bin, lag_values);
    }

    Ok(sync2d_map)
}

#[test]
#[ignore] // Run with: cargo test test_sync2d -- --ignored
fn test_sync2d_matches_wsjtx() {
    init_test_tracing();

    // Load the same WAV file used for WSJT-X reference
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read test WAV file");
    let signal = normalize_signal_length(signal);

    println!("RustyFt8 Sync2d Validation Test");
    println!("================================");
    println!();

    // Compute spectra
    let mut spectra = vec![[0.0f32; NHSYM]; NH1];
    let _avg_spectrum = compute_spectra(&signal, &mut spectra)
        .expect("compute_spectra should succeed");

    // Compute sync2d
    let mut sync2d = Vec::new();
    let (ia, ib) = compute_sync2d(&spectra, 200.0, 4000.0, &mut sync2d)
        .expect("compute_sync2d should succeed");

    println!("Computed sync2d for bins {} to {}", ia, ib);
    println!();

    // Load WSJT-X reference data
    println!("Loading WSJT-X reference sync2d from CSV...");
    let ref_sync2d = read_sync2d_csv("tests/sync/sync2d_ref.csv")
        .expect("Failed to read sync2d_ref.csv");
    println!("Loaded {} bins from reference", ref_sync2d.len());
    println!();

    // Compare key bins where WSJT-X found candidates
    let test_cases = vec![
        (477, 1, "1490.6 Hz - peak at lag 1"),
        (478, 2, "1493.8 Hz - peak at lag 2"),
        (482, 10, "1506.2 Hz - peak at lag 10"),
        (823, 7, "2571.9 Hz - peak at lag 7"),  // Corrected to lag 7 (actual peak)
        (811, 59, "2534.4 Hz - peak at lag 59"), // Corrected to lag 59 (actual peak)
    ];

    println!("Comparing sync2d values at key bins:");
    println!("=====================================");

    for &(bin, expected_peak_lag, desc) in &test_cases {
        if let Some(ref_values) = ref_sync2d.get(&bin) {
            println!();
            println!("Bin {} - {}", bin, desc);
            println!("  {:<6} {:<12} {:<12} {:<12} {:<12}", "lag", "WSJT-X", "RustyFt8", "Diff", "Rel Err %");
            println!("  {}", "-".repeat(60));

            // Compare values around the expected peak
            let start_lag = (expected_peak_lag - 3).max(-MAX_LAG);
            let end_lag = (expected_peak_lag + 3).min(MAX_LAG);

            for lag in start_lag..=end_lag {
                let ref_idx = (lag + MAX_LAG) as usize;
                let expected = ref_values[ref_idx];
                let actual = sync2d[bin][ref_idx];

                let diff = (actual - expected).abs();
                let rel_error = if expected.abs() > 1e-10 {
                    diff / expected.abs() * 100.0
                } else if actual.abs() < 1e-10 {
                    0.0
                } else {
                    100.0  // Expected zero but got non-zero
                };

                let marker = if lag == expected_peak_lag { " ← PEAK" } else { "" };
                println!("  {:<6} {:<12.6} {:<12.6} {:<12.6} {:<12.2}{}",
                         lag, expected, actual, diff, rel_error, marker);
            }

            // Find actual peaks
            let rust_peak_lag = sync2d[bin].iter().enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx as i32 - MAX_LAG)
                .unwrap_or(0);

            let wsjtx_peak_lag = ref_values.iter().enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(idx, _)| idx as i32 - MAX_LAG)
                .unwrap_or(0);

            println!();
            println!("  Peak detection:");
            println!("    WSJT-X:   lag={} sync={:.6}", wsjtx_peak_lag, ref_values[(wsjtx_peak_lag + MAX_LAG) as usize]);
            println!("    RustyFt8: lag={} sync={:.6}", rust_peak_lag, sync2d[bin][(rust_peak_lag + MAX_LAG) as usize]);

            if rust_peak_lag == wsjtx_peak_lag {
                println!("    ✓ Peak lag matches!");
            } else {
                println!("    ✗ Peak lag mismatch: {} vs {}", rust_peak_lag, wsjtx_peak_lag);
            }
        } else {
            println!("Bin {} not found in reference data", bin);
        }
    }

    // Comprehensive comparison statistics
    println!();
    println!("Comprehensive Comparison:");
    println!("=========================");

    let mut total_compared = 0;
    let mut perfect_matches = 0;
    let mut good_matches = 0;
    let mut acceptable_matches = 0;
    let mut poor_matches = 0;
    let mut max_rel_error = 0.0f32;
    let mut max_error_location = (0, 0);

    for (&bin, ref_values) in &ref_sync2d {
        for lag_idx in 0..ref_values.len() {
            let expected = ref_values[lag_idx];
            let actual = sync2d[bin][lag_idx];

            let diff = (actual - expected).abs();
            let rel_error = if expected.abs() > 1e-10 {
                diff / expected.abs()
            } else if actual.abs() < 1e-10 {
                0.0
            } else {
                1.0
            };

            total_compared += 1;

            if rel_error < 0.00001 {
                perfect_matches += 1;
            } else if rel_error < 0.0001 {
                good_matches += 1;
            } else if rel_error < 0.01 {
                acceptable_matches += 1;
            } else {
                poor_matches += 1;
            }

            if rel_error > max_rel_error {
                max_rel_error = rel_error;
                max_error_location = (bin, lag_idx as i32 - MAX_LAG);
            }
        }
    }

    println!("  Total values:       {}", total_compared);
    println!("  Perfect (<0.001%):  {} ({:.2}%)", perfect_matches, 100.0 * perfect_matches as f32 / total_compared as f32);
    println!("  Good (<0.01%):      {} ({:.2}%)", good_matches, 100.0 * good_matches as f32 / total_compared as f32);
    println!("  Acceptable (<1%):   {} ({:.2}%)", acceptable_matches, 100.0 * acceptable_matches as f32 / total_compared as f32);
    println!("  Poor (>=1%):        {} ({:.2}%)", poor_matches, 100.0 * poor_matches as f32 / total_compared as f32);
    println!("  Max relative error: {:.6} at bin={}, lag={}", max_rel_error, max_error_location.0, max_error_location.1);
    println!();

    // Assert that sync2d values match well
    let within_point01_ratio = (perfect_matches + good_matches) as f32 / total_compared as f32;
    let within_1pct_ratio = (total_compared - poor_matches) as f32 / total_compared as f32;

    assert!(
        within_point01_ratio > 0.99,
        "Only {:.4}% of sync2d values match within 0.01% (expected >99%)",
        within_point01_ratio * 100.0
    );

    assert!(
        within_1pct_ratio > 0.999,
        "Only {:.4}% of sync2d values match within 1% (expected >99.9%)",
        within_1pct_ratio * 100.0
    );

    println!("✅ SUCCESS: Sync2d computation validated!");
    println!("  {:.2}% match within 0.01%", within_point01_ratio * 100.0);
    println!("  {:.2}% match within 1%", within_1pct_ratio * 100.0);
}
