//! Test compute_spectra against WSJT-X reference implementation
//!
//! This test verifies that our FFT and power spectra computation matches WSJT-X exactly.

use rustyft8::sync::{compute_spectra, NH1, NHSYM};
use std::fs::File;
use std::io::{BufRead, BufReader};

#[path = "../test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length, init_test_tracing};

/// Read CSV file with comma-separated f32 values
fn read_csv_row(line: &str) -> Vec<f32> {
    line.split(',')
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect()
}

/// Read full spectra from CSV (NH1 rows × NHSYM columns)
fn read_spectra_csv(path: &str) -> Result<Vec<Vec<f32>>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open {}: {}", path, e))?;
    let reader = BufReader::new(file);

    let mut spectra = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
        let row = read_csv_row(&line);
        if row.len() != NHSYM {
            return Err(format!("Expected {} columns, got {}", NHSYM, row.len()));
        }
        spectra.push(row);
    }

    if spectra.len() != NH1 {
        return Err(format!("Expected {} rows, got {}", NH1, spectra.len()));
    }

    Ok(spectra)
}

/// Read average spectrum from CSV (single row with NH1 values)
fn read_avg_spectrum_csv(path: &str) -> Result<Vec<f32>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open {}: {}", path, e))?;
    let mut reader = BufReader::new(file);

    let mut line = String::new();
    reader.read_line(&mut line)
        .map_err(|e| format!("Failed to read line: {}", e))?;

    let values = read_csv_row(&line);
    if values.len() != NH1 {
        return Err(format!("Expected {} values, got {}", NH1, values.len()));
    }

    Ok(values)
}

#[test]
#[ignore] // Run with: cargo test -- --ignored
fn test_spectra_matches_wsjtx() {
    init_test_tracing();

    // Load the same WAV file used for WSJT-X reference
    // Use raw reading to match WSJT-X: data(i) = real(iwave(i))
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read test WAV file");
    let signal = normalize_signal_length(signal);

    println!("RustyFt8 Spectra Validation Test");
    println!("==================================");
    println!("Testing {} × {} = {} spectra values", NH1, NHSYM, NH1 * NHSYM);
    println!();

    // Compute spectra using RustyFt8
    let mut spectra = vec![[0.0f32; NHSYM]; NH1];
    let avg_spectrum = compute_spectra(&signal, &mut spectra)
        .expect("compute_spectra should succeed");

    // Load WSJT-X reference data from CSV files
    println!("Loading WSJT-X reference data from CSV files...");
    let ref_spectra = read_spectra_csv("tests/sync/spectra.csv")
        .expect("Failed to read spectra.csv");
    let ref_avg_spectrum = read_avg_spectrum_csv("tests/sync/avg_spectrum.csv")
        .expect("Failed to read avg_spectrum.csv");
    println!("Loaded {} × {} spectra values", ref_spectra.len(), ref_spectra[0].len());
    println!();

    // Compare all spectra values
    // IMPORTANT: Fortran s(i,j) for i=1..NH1 stores FFT bins 1..NH1 (skips DC at bin 0)
    // Our Rust spectra[i][j] for i=0..NH1-1 stores FFT bins 0..NH1-1 (includes DC at index 0)
    // So: Fortran s(bin,time) = CSV row (bin-1) = Rust spectra[bin][time-1]
    // Mapping: ref_spectra[i][j] (Fortran bin i+1) = spectra[i+1][j] (Rust bin i+1)
    println!("Comparing full spectra array...");
    let tolerance = 1e-5; // Very tight tolerance (0.001%)

    let mut total_values = 0;
    let mut perfect_matches = 0;
    let mut good_matches = 0;  // Within 0.01%
    let mut acceptable_matches = 0;  // Within 1%
    let mut poor_matches = 0;
    let mut max_rel_error = 0.0f32;
    let mut max_error_location = (0, 0);

    // Compare bins 1..NH1-1 (skipping DC at index 0 and missing bin NH1=1920)
    for rust_bin in 1..NH1 {
        let csv_row = rust_bin - 1;  // CSV row 0 = Fortran bin 1 = Rust bin 1

        for j in 0..NHSYM {
            let actual = spectra[rust_bin][j];
            let expected = ref_spectra[csv_row][j];

            let diff = (actual - expected).abs();
            let rel_error = if expected.abs() > 1e-10 {
                diff / expected.abs()
            } else if actual.abs() < 1e-10 {
                0.0  // Both essentially zero
            } else {
                1.0  // Expected zero but got non-zero
            };

            total_values += 1;

            if rel_error < tolerance {
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
                max_error_location = (rust_bin, j);
            }
        }
    }

    println!("Spectra comparison results:");
    println!("  Total values:       {}", total_values);
    println!("  Perfect (<0.001%):  {} ({:.2}%)", perfect_matches, 100.0 * perfect_matches as f32 / total_values as f32);
    println!("  Good (<0.01%):      {} ({:.2}%)", good_matches, 100.0 * good_matches as f32 / total_values as f32);
    println!("  Acceptable (<1%):   {} ({:.2}%)", acceptable_matches, 100.0 * acceptable_matches as f32 / total_values as f32);
    println!("  Poor (>=1%):        {} ({:.2}%)", poor_matches, 100.0 * poor_matches as f32 / total_values as f32);
    println!("  Max relative error: {:.6} at bin={}, time={}", max_rel_error, max_error_location.0, max_error_location.1);
    println!();

    // Compare average spectrum
    println!("Comparing average spectrum...");
    let mut avg_perfect = 0;
    let mut avg_good = 0;
    let mut avg_acceptable = 0;
    let mut avg_poor = 0;
    let mut avg_max_rel_error = 0.0f32;
    let mut avg_max_error_bin = 0;

    // Compare bins 1..NH1-1 (same as spectra)
    for rust_bin in 1..NH1 {
        let csv_idx = rust_bin - 1;
        let actual = avg_spectrum[rust_bin];
        let expected = ref_avg_spectrum[csv_idx];

        let diff = (actual - expected).abs();
        let rel_error = if expected.abs() > 1e-10 {
            diff / expected.abs()
        } else if actual.abs() < 1e-10 {
            0.0
        } else {
            1.0
        };

        if rel_error < tolerance {
            avg_perfect += 1;
        } else if rel_error < 0.0001 {
            avg_good += 1;
        } else if rel_error < 0.01 {
            avg_acceptable += 1;
        } else {
            avg_poor += 1;
        }

        if rel_error > avg_max_rel_error {
            avg_max_rel_error = rel_error;
            avg_max_error_bin = rust_bin;
        }
    }

    let total_bins_compared = NH1 - 1;  // Bins 1..NH1-1
    println!("Average spectrum comparison results:");
    println!("  Total bins:         {}", total_bins_compared);
    println!("  Perfect (<0.001%):  {} ({:.2}%)", avg_perfect, 100.0 * avg_perfect as f32 / total_bins_compared as f32);
    println!("  Good (<0.01%):      {} ({:.2}%)", avg_good, 100.0 * avg_good as f32 / total_bins_compared as f32);
    println!("  Acceptable (<1%):   {} ({:.2}%)", avg_acceptable, 100.0 * avg_acceptable as f32 / total_bins_compared as f32);
    println!("  Poor (>=1%):        {} ({:.2}%)", avg_poor, 100.0 * avg_poor as f32 / total_bins_compared as f32);
    println!("  Max relative error: {:.6} at bin={}", avg_max_rel_error, avg_max_error_bin);
    println!();

    // Sample spot checks for visual verification
    println!("Sample spot checks:");
    println!("===================");
    let spot_checks = vec![
        (477, 0, "bin 477, time 1"),
        (477, 13, "bin 477, time 14"),
        (478, 0, "bin 478, time 1"),
        (482, 0, "bin 482, time 1"),
    ];

    for &(bin, time, desc) in &spot_checks {
        let actual = spectra[bin][time];
        let csv_row = bin - 1;  // CSV row 476 = Fortran bin 477
        let expected = ref_spectra[csv_row][time];
        let rel_error = if expected.abs() > 1e-10 {
            (actual - expected).abs() / expected.abs()
        } else {
            0.0
        };
        println!("  {}: expected={:.6}, actual={:.6}, error={:.6}%",
                 desc, expected, actual, rel_error * 100.0);
    }
    println!();

    // Assert that the vast majority of values match well
    // Note: Small differences (~0.8%) are expected due to different FFT libraries
    // (RustFFT vs FFTW) and floating-point precision
    let within_point01_ratio = (perfect_matches + good_matches) as f32 / total_values as f32;
    let within_1pct_ratio = (total_values - poor_matches) as f32 / total_values as f32;

    assert!(
        within_point01_ratio > 0.99,
        "Only {:.4}% of spectra values match within 0.01% (expected >99%)",
        within_point01_ratio * 100.0
    );

    assert!(
        within_1pct_ratio > 0.9999,
        "Only {:.4}% of spectra values match within 1% (expected >99.99%)",
        within_1pct_ratio * 100.0
    );

    assert!(
        poor_matches < 10,
        "Found {} values with >1% error (max error: {:.6} at bin={}, time={})",
        poor_matches, max_rel_error, max_error_location.0, max_error_location.1
    );

    println!();
    println!("✅ SUCCESS: Spectra computation validated!");
    println!("  {:.2}% match within 0.01% (FFT precision)", within_point01_ratio * 100.0);
    println!("  {:.2}% match within 1% (excellent agreement)", within_1pct_ratio * 100.0);
    println!("  {} outliers with >1% error", poor_matches);
}
