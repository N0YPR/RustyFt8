//! Export coarse sync candidates to CSV for fine sync validation

use rustyft8::sync::coarse_sync;
use std::fs::File;
use std::io::Write;

#[path = "../test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn export_coarse_candidates() {
    // Load test WAV file
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    // Run coarse sync (refactored API does everything internally)
    let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("Exporting {} candidates to tests/sync/coarse_candidates.csv", candidates.len());

    // Write to CSV
    let mut file = File::create("tests/sync/coarse_candidates.csv")
        .expect("Failed to create CSV file");

    writeln!(file, "frequency,time_offset,sync_power,baseline_noise")
        .expect("Failed to write header");

    for cand in &candidates {
        writeln!(
            file,
            "{:.1},{:.3},{:.3},{:.3}",
            cand.frequency,
            cand.time_offset,
            cand.sync_power,
            cand.baseline_noise
        ).expect("Failed to write candidate");
    }

    println!("Successfully exported {} candidates", candidates.len());
}
