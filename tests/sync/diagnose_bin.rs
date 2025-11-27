//! Diagnostic tool to understand why specific WSJT-X candidates are missing

use rustyft8::sync::{compute_spectra, compute_sync2d, NFFT1, SAMPLE_RATE, NH1, MAX_LAG};

#[path = "../test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length, init_test_tracing};

#[test]
#[ignore]
fn diagnose_missing_candidates() {
    init_test_tracing();
    // Load WAV file
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    // Compute spectra and sync2d
    let mut spectra = vec![[0.0f32; rustyft8::sync::NHSYM]; NH1];
    compute_spectra(&signal, &mut spectra).expect("spectra failed");

    let mut sync2d = Vec::new();
    let (ia, ib) = compute_sync2d(&spectra, 200.0, 4000.0, &mut sync2d)
        .expect("sync2d failed");

    let df = SAMPLE_RATE / NFFT1 as f32;
    let tstep = rustyft8::sync::NSTEP as f32 / SAMPLE_RATE;

    // Target candidates from WSJT-X that we're missing
    let missing_candidates = vec![
        (2609.4, -0.500, 32.226),  // freq, time, sync
        (1490.6, 0.020, 1.569),
        (1493.8, 0.060, 1.288),
    ];

    println!("Diagnosing Missing WSJT-X Candidates");
    println!("====================================\n");

    for (freq, time, expected_sync) in &missing_candidates {
        let bin = (freq / df).round() as usize;
        let expected_lag = ((time + 0.5) / tstep).round() as i32;

        println!("WSJT-X candidate: {:.1} Hz, {:.3} s, sync={:.3}", freq, time, expected_sync);
        println!("  Bin: {}, Expected lag: {}", bin, expected_lag);

        if bin < ia || bin > ib {
            println!("  ❌ Bin is OUTSIDE search range ({}-{})", ia, ib);
            continue;
        }

        // Show sync2d values around expected lag
        println!("\n  Sync2d values at bin {} (freq {:.1} Hz):", bin, bin as f32 * df);
        println!("    Lag    Sync2d    Time");
        println!("    ---    -------   -----");
        for lag in (expected_lag - 5)..=(expected_lag + 5) {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[bin].len() {
                let time_offset = (lag as f32 - 0.5) * tstep;
                let marker = if lag == expected_lag { " ← expected" } else { "" };
                println!("    {:3}    {:.4}    {:.3}{}", lag, sync2d[bin][sync_idx], time_offset, marker);
            }
        }

        // Find actual peak in narrow range (±10 lags)
        let mut narrow_peak_lag = 0;
        let mut narrow_peak_sync = 0.0f32;
        for lag in -10..=10 {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[bin].len() && sync2d[bin][sync_idx] > narrow_peak_sync {
                narrow_peak_sync = sync2d[bin][sync_idx];
                narrow_peak_lag = lag;
            }
        }

        // Find actual peak in wide range (±62 lags)
        let mut wide_peak_lag = 0;
        let mut wide_peak_sync = 0.0f32;
        for lag in -MAX_LAG..=MAX_LAG {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[bin].len() && sync2d[bin][sync_idx] > wide_peak_sync {
                wide_peak_sync = sync2d[bin][sync_idx];
                wide_peak_lag = lag;
            }
        }

        println!("\n  Actual peaks at bin {}:", bin);
        println!("    Narrow (±10): lag={}, sync={:.4}, time={:.3}",
                 narrow_peak_lag, narrow_peak_sync, (narrow_peak_lag as f32 - 0.5) * tstep);
        println!("    Wide (±62):   lag={}, sync={:.4}, time={:.3}",
                 wide_peak_lag, wide_peak_sync, (wide_peak_lag as f32 - 0.5) * tstep);

        println!();
    }
}
