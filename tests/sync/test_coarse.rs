//! Integration test to verify candidate detection coverage
//!
//! This test ensures that coarse_sync() detects all signals that WSJT-X successfully decodes.
//! Uses real FT8 recording validated against WSJT-X jt9 output.

use rustyft8::sync::coarse_sync;

#[path = "../test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file, normalize_signal_length};

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_coarse_sync_matches_wsjtx() {
    // This test compares RustyFt8's coarse_sync output against WSJT-X's sync8 function.
    // WSJT-X sync8 is the reference implementation for FT8 coarse synchronization.
    //
    // HOW THIS REFERENCE DATA WAS OBTAINED:
    //
    // 1. Created a Fortran test program (tests/sync/test_sync8.f90) that calls WSJT-X's sync8:
    //    - Reads the same test WAV file: tests/test_data/210703_133430.wav
    //    - Calls sync8 with parameters: nfa=200, nfb=3500, syncmin=0.3, maxcand=200
    //    - Outputs all candidates with (frequency, time_offset, sync_power)
    //
    // 2. Compiled against WSJT-X libraries:
    //    cd /workspaces/RustyFt8
    //    gfortran tests/sync/test_sync8.f90 \
    //             wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/libwsjt_fort.a \
    //             -o test_sync8 -lfftw3f -lm -O2
    //
    // 3. Ran the program to generate reference output:
    //    ./test_sync8 > wsjtx_sync8_output.txt
    //
    // 4. Parsed the output into the Vec below
    //
    // This ensures we're comparing against the EXACT output of WSJT-X's reference implementation,
    // not just decoded messages or manually picked frequencies.
    //
    // Expected candidates from WSJT-X sync8 (200 candidates with syncmin=0.3):

    let wsjtx_candidates: Vec<(f32, f32, f32)> = vec![
            (1490.6, 0.020, 1.569),  // freq, time_offset, sync_power
            (1493.8, 0.060, 1.288),  // freq, time_offset, sync_power
            (1493.8, 1.820, 1.380),  // freq, time_offset, sync_power
            (1506.2, 0.380, 1.254),  // freq, time_offset, sync_power
            (1506.2, 1.620, 1.421),  // freq, time_offset, sync_power
            (1503.1, -0.100, 1.120),  // freq, time_offset, sync_power
            (1503.1, -1.140, 0.941),  // freq, time_offset, sync_power
            (1500.0, 0.860, 1.031),  // freq, time_offset, sync_power
            (2571.9, 0.300, 237.840),  // freq, time_offset, sync_power
            (2534.4, 2.380, 67.523),  // freq, time_offset, sync_power
            (2540.6, 2.220, 60.383),  // freq, time_offset, sync_power
            (2534.4, 0.140, 53.595),  // freq, time_offset, sync_power
            (2609.4, -0.500, 32.226),  // freq, time_offset, sync_power
            (2156.2, -0.020, 31.801),  // freq, time_offset, sync_power
            (2118.8, 2.220, 25.980),  // freq, time_offset, sync_power
            (2196.9, 0.140, 18.923),  // freq, time_offset, sync_power
            (2203.1, -1.140, 16.903),  // freq, time_offset, sync_power
            (2143.8, 1.900, 16.358),  // freq, time_offset, sync_power
            (590.6, 0.340, 15.809),  // freq, time_offset, sync_power
            (1159.4, -0.900, 13.229),  // freq, time_offset, sync_power
            (721.9, 0.180, 12.993),  // freq, time_offset, sync_power
            (2696.9, -0.100, 12.741),  // freq, time_offset, sync_power
            (428.1, 0.140, 10.988),  // freq, time_offset, sync_power
            (2125.0, 2.060, 10.805),  // freq, time_offset, sync_power
            (640.6, 0.300, 10.726),  // freq, time_offset, sync_power
            (2596.9, -2.420, 10.655),  // freq, time_offset, sync_power
            (465.6, 0.300, 10.162),  // freq, time_offset, sync_power
            (1390.6, 0.140, 9.880),  // freq, time_offset, sync_power
            (2237.5, 0.300, 9.374),  // freq, time_offset, sync_power
            (1190.6, 0.220, 9.178),  // freq, time_offset, sync_power
            (2531.2, 0.100, 9.039),  // freq, time_offset, sync_power
            (762.5, 0.340, 8.846),  // freq, time_offset, sync_power
            (684.4, 0.020, 7.619),  // freq, time_offset, sync_power
            (1650.0, 0.140, 7.503),  // freq, time_offset, sync_power
            (2734.4, 0.460, 6.644),  // freq, time_offset, sync_power
            (1184.4, -0.100, 6.218),  // freq, time_offset, sync_power
            (2590.6, -2.100, 5.880),  // freq, time_offset, sync_power
            (440.6, -0.820, 5.667),  // freq, time_offset, sync_power
            (400.0, 0.300, 5.610),  // freq, time_offset, sync_power
            (2856.2, 0.220, 5.413),  // freq, time_offset, sync_power
            (2118.8, -0.180, 5.304),  // freq, time_offset, sync_power
            (1178.1, -2.180, 5.105),  // freq, time_offset, sync_power
            (434.4, -0.660, 5.013),  // freq, time_offset, sync_power
            (503.1, 0.460, 4.939),  // freq, time_offset, sync_power
            (1165.6, 0.060, 4.645),  // freq, time_offset, sync_power
            (1221.9, 0.380, 4.642),  // freq, time_offset, sync_power
            (1165.6, 1.980, 4.562),  // freq, time_offset, sync_power
            (2140.6, 1.100, 4.388),  // freq, time_offset, sync_power
            (1234.4, -0.580, 4.337),  // freq, time_offset, sync_power
            (509.4, 2.380, 4.336),  // freq, time_offset, sync_power
            (2537.5, 0.980, 4.033),  // freq, time_offset, sync_power
            (1234.4, 0.220, 4.025),  // freq, time_offset, sync_power
            (2190.6, 0.260, 4.011),  // freq, time_offset, sync_power
            (1200.0, -0.740, 3.922),  // freq, time_offset, sync_power
            (2184.4, 0.940, 3.748),  // freq, time_offset, sync_power
            (425.0, 0.100, 3.746),  // freq, time_offset, sync_power
            (2668.8, 0.260, 3.676),  // freq, time_offset, sync_power
            (2590.6, -0.180, 3.644),  // freq, time_offset, sync_power
            (425.0, 1.940, 3.634),  // freq, time_offset, sync_power
            (696.9, 1.620, 3.627),  // freq, time_offset, sync_power
            (1434.4, 0.940, 3.617),  // freq, time_offset, sync_power
            (515.6, -0.300, 3.577),  // freq, time_offset, sync_power
            (2281.2, 0.140, 3.575),  // freq, time_offset, sync_power
            (2131.2, 0.300, 3.549),  // freq, time_offset, sync_power
            (515.6, 1.740, 3.458),  // freq, time_offset, sync_power
            (753.1, -0.300, 3.458),  // freq, time_offset, sync_power
            (1171.9, 1.820, 3.406),  // freq, time_offset, sync_power
            (2115.6, -0.220, 3.396),  // freq, time_offset, sync_power
            (1228.1, -1.060, 3.349),  // freq, time_offset, sync_power
            (453.1, 0.940, 3.330),  // freq, time_offset, sync_power
            (2740.6, 2.340, 3.184),  // freq, time_offset, sync_power
            (709.4, 0.820, 3.118),  // freq, time_offset, sync_power
            (2150.0, 0.940, 3.109),  // freq, time_offset, sync_power
            (1415.6, 1.420, 3.103),  // freq, time_offset, sync_power
            (2731.2, 1.660, 3.092),  // freq, time_offset, sync_power
            (1240.6, 1.660, 3.062),  // freq, time_offset, sync_power
            (2168.8, -2.260, 3.056),  // freq, time_offset, sync_power
            (553.1, 0.180, 3.048),  // freq, time_offset, sync_power
            (2678.1, -1.540, 3.037),  // freq, time_offset, sync_power
            (2671.9, 1.540, 3.017),  // freq, time_offset, sync_power
            (1240.6, -0.100, 3.009),  // freq, time_offset, sync_power
            (2728.1, 1.860, 2.991),  // freq, time_offset, sync_power
            (2821.9, 1.780, 2.933),  // freq, time_offset, sync_power
            (496.9, 1.580, 2.922),  // freq, time_offset, sync_power
            (2484.4, 0.940, 2.917),  // freq, time_offset, sync_power
            (690.6, -0.140, 2.904),  // freq, time_offset, sync_power
            (603.1, -1.420, 2.842),  // freq, time_offset, sync_power
            (1206.2, 0.100, 2.817),  // freq, time_offset, sync_power
            (1453.1, 0.460, 2.801),  // freq, time_offset, sync_power
            (1212.5, 0.020, 2.799),  // freq, time_offset, sync_power
            (1228.1, -0.220, 2.788),  // freq, time_offset, sync_power
            (2737.5, 1.500, 2.773),  // freq, time_offset, sync_power
            (728.1, 2.100, 2.751),  // freq, time_offset, sync_power
            (712.5, -0.300, 2.751),  // freq, time_offset, sync_power
            (2171.9, 1.580, 2.724),  // freq, time_offset, sync_power
            (2684.4, 0.540, 2.717),  // freq, time_offset, sync_power
            (703.1, 1.460, 2.703),  // freq, time_offset, sync_power
            (2659.4, 2.340, 2.654),  // freq, time_offset, sync_power
            (2828.1, 1.660, 2.646),  // freq, time_offset, sync_power
            (459.4, -0.980, 2.634),  // freq, time_offset, sync_power
            (446.9, -0.340, 2.609),  // freq, time_offset, sync_power
            (2746.9, 1.260, 2.608),  // freq, time_offset, sync_power
            (734.4, -0.460, 2.592),  // freq, time_offset, sync_power
            (2665.6, 1.700, 2.565),  // freq, time_offset, sync_power
            (559.4, -0.140, 2.562),  // freq, time_offset, sync_power
            (1431.2, 0.300, 2.562),  // freq, time_offset, sync_power
            (1684.4, -1.460, 2.541),  // freq, time_offset, sync_power
            (1175.0, 1.860, 2.529),  // freq, time_offset, sync_power
            (2584.4, -2.260, 2.529),  // freq, time_offset, sync_power
            (2525.0, 0.260, 2.527),  // freq, time_offset, sync_power
            (1468.8, -2.260, 2.514),  // freq, time_offset, sync_power
            (718.8, 1.140, 2.507),  // freq, time_offset, sync_power
            (637.5, 0.340, 2.500),  // freq, time_offset, sync_power
            (1425.0, -2.420, 2.496),  // freq, time_offset, sync_power
            (2846.9, 1.460, 2.492),  // freq, time_offset, sync_power
            (1171.9, -0.420, 2.487),  // freq, time_offset, sync_power
            (2690.6, 1.380, 2.486),  // freq, time_offset, sync_power
            (700.0, 2.420, 2.478),  // freq, time_offset, sync_power
            (2759.4, 1.060, 2.474),  // freq, time_offset, sync_power
            (2840.6, 2.460, 2.472),  // freq, time_offset, sync_power
            (375.0, -2.100, 2.465),  // freq, time_offset, sync_power
            (2540.6, -0.020, 2.448),  // freq, time_offset, sync_power
            (2496.9, -0.820, 2.442),  // freq, time_offset, sync_power
            (1215.6, -0.060, 2.438),  // freq, time_offset, sync_power
            (584.4, 1.300, 2.434),  // freq, time_offset, sync_power
            (2565.6, 1.740, 2.433),  // freq, time_offset, sync_power
            (2603.1, 0.780, 2.431),  // freq, time_offset, sync_power
            (1356.2, -0.020, 2.420),  // freq, time_offset, sync_power
            (1159.4, 0.220, 2.417),  // freq, time_offset, sync_power
            (393.8, -2.260, 2.411),  // freq, time_offset, sync_power
            (2675.0, 0.100, 2.410),  // freq, time_offset, sync_power
            (2765.6, 2.220, 2.406),  // freq, time_offset, sync_power
            (2134.4, -1.620, 2.395),  // freq, time_offset, sync_power
            (2503.1, -0.340, 2.380),  // freq, time_offset, sync_power
            (750.0, -0.140, 2.372),  // freq, time_offset, sync_power
            (2765.6, -0.340, 2.372),  // freq, time_offset, sync_power
            (1178.1, -0.260, 2.368),  // freq, time_offset, sync_power
            (1231.2, -0.500, 2.354),  // freq, time_offset, sync_power
            (362.5, 0.140, 2.346),  // freq, time_offset, sync_power
            (756.2, 0.660, 2.340),  // freq, time_offset, sync_power
            (740.6, 0.860, 2.335),  // freq, time_offset, sync_power
            (446.9, 0.780, 2.329),  // freq, time_offset, sync_power
            (2725.0, 1.740, 2.322),  // freq, time_offset, sync_power
            (1615.6, -1.460, 2.309),  // freq, time_offset, sync_power
            (909.4, 0.180, 2.307),  // freq, time_offset, sync_power
            (1209.4, 1.020, 2.299),  // freq, time_offset, sync_power
            (678.1, 2.380, 2.286),  // freq, time_offset, sync_power
            (1215.6, 1.380, 2.275),  // freq, time_offset, sync_power
            (746.9, -0.100, 2.262),  // freq, time_offset, sync_power
            (2181.2, 0.620, 2.255),  // freq, time_offset, sync_power
            (2737.5, 0.380, 2.253),  // freq, time_offset, sync_power
            (431.2, 0.100, 2.252),  // freq, time_offset, sync_power
            (2137.5, -0.180, 2.244),  // freq, time_offset, sync_power
            (2612.5, 2.220, 2.242),  // freq, time_offset, sync_power
            (2871.9, 1.660, 2.230),  // freq, time_offset, sync_power
            (737.5, 1.740, 2.229),  // freq, time_offset, sync_power
            (406.2, -1.780, 2.215),  // freq, time_offset, sync_power
            (765.6, 0.260, 2.209),  // freq, time_offset, sync_power
            (1409.4, -0.020, 2.207),  // freq, time_offset, sync_power
            (2890.6, -0.540, 2.168),  // freq, time_offset, sync_power
            (1428.1, 0.340, 2.149),  // freq, time_offset, sync_power
            (2865.6, -2.140, 2.141),  // freq, time_offset, sync_power
            (1684.4, 0.340, 2.139),  // freq, time_offset, sync_power
            (462.5, 0.340, 2.128),  // freq, time_offset, sync_power
            (2178.1, -0.220, 2.128),  // freq, time_offset, sync_power
            (2728.1, 0.220, 2.113),  // freq, time_offset, sync_power
            (503.1, -0.340, 2.112),  // freq, time_offset, sync_power
            (2165.6, 0.780, 2.090),  // freq, time_offset, sync_power
            (484.4, 1.300, 2.088),  // freq, time_offset, sync_power
            (368.8, -1.940, 2.088),  // freq, time_offset, sync_power
            (1421.9, -2.460, 2.082),  // freq, time_offset, sync_power
            (490.6, 0.940, 2.077),  // freq, time_offset, sync_power
            (2175.0, 0.660, 2.077),  // freq, time_offset, sync_power
            (2593.8, 0.060, 2.073),  // freq, time_offset, sync_power
            (2153.1, 0.020, 2.069),  // freq, time_offset, sync_power
            (2703.1, 0.380, 2.062),  // freq, time_offset, sync_power
            (1378.1, 0.780, 2.055),  // freq, time_offset, sync_power
            (2181.2, -0.300, 2.052),  // freq, time_offset, sync_power
            (1687.5, 0.300, 2.032),  // freq, time_offset, sync_power
            (2546.9, -0.340, 2.026),  // freq, time_offset, sync_power
            (2509.4, -0.020, 2.025),  // freq, time_offset, sync_power
            (2721.9, 0.540, 2.015),  // freq, time_offset, sync_power
            (2596.9, -0.020, 2.010),  // freq, time_offset, sync_power
            (2759.4, 0.140, 2.007),  // freq, time_offset, sync_power
            (1225.0, -0.140, 2.005),  // freq, time_offset, sync_power
            (440.6, -0.340, 2.002),  // freq, time_offset, sync_power
            (2687.5, 0.900, 2.001),  // freq, time_offset, sync_power
            (571.9, -0.940, 1.994),  // freq, time_offset, sync_power
            (1384.4, 2.420, 1.993),  // freq, time_offset, sync_power
            (490.6, -0.180, 1.989),  // freq, time_offset, sync_power
            (512.5, 0.660, 1.983),  // freq, time_offset, sync_power
            (250.0, 1.340, 1.981),  // freq, time_offset, sync_power
            (703.1, 0.340, 1.980),  // freq, time_offset, sync_power
            (743.8, -0.020, 1.979),  // freq, time_offset, sync_power
            (2546.9, -0.980, 1.973),  // freq, time_offset, sync_power
            (546.9, 2.260, 1.964),  // freq, time_offset, sync_power
            (1181.2, -2.140, 1.960),  // freq, time_offset, sync_power
            (2718.8, -0.340, 1.956),  // freq, time_offset, sync_power
            (2743.8, -0.180, 1.952),  // freq, time_offset, sync_power
            (681.2, -0.060, 1.947),  // freq, time_offset, sync_power
        
    ];

    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV file");

    let signal_15s = normalize_signal_length(signal);

    // Run coarse_sync with same parameters as WSJT-X sync8
    let rust_candidates = coarse_sync(
        &signal_15s,
        200.0,   // nfa (freq_min)
        3500.0,  // nfb (freq_max)
        0.3,     // syncmin
        200,     // maxcand
    ).expect("Coarse sync failed");

    println!("\nWSJT-X found {} candidates", wsjtx_candidates.len());
    println!("RustyFt8 found {} candidates", rust_candidates.len());

    // Require exact same number of candidates
    assert_eq!(rust_candidates.len(), wsjtx_candidates.len(),
               "RustyFt8 found {} candidates, WSJT-X found {} - must match exactly",
               rust_candidates.len(), wsjtx_candidates.len());

    // Strict tolerances for comparison - these should match the reference implementation closely
    let freq_tolerance = 3.0;   // ±3 Hz (less than one frequency bin)
    let time_tolerance = 0.02;   // ±20 ms (half a symbol time step)
    let sync_rel_tolerance = 0.05; // ±5% relative error for sync power

    let mut matched_count = 0;
    let mut mismatched = Vec::new();

    for (wsjtx_freq, wsjtx_time, wsjtx_sync) in &wsjtx_candidates {
        // Find matching candidate in RustyFt8 output
        let match_found = rust_candidates.iter().any(|rc| {
            let freq_match = (rc.frequency - wsjtx_freq).abs() <= freq_tolerance;
            let time_match = (rc.time_offset - wsjtx_time).abs() <= time_tolerance;
            
            // Sync power comparison: allow relative or absolute tolerance
            let sync_diff = (rc.sync_power - wsjtx_sync).abs();
            let sync_rel_diff = sync_diff / wsjtx_sync.max(0.001);  // Avoid div by zero
            let sync_match = sync_rel_diff <= sync_rel_tolerance || sync_diff <= 0.5;

            freq_match && time_match && sync_match
        });

        if match_found {
            matched_count += 1;
        } else {
            mismatched.push((*wsjtx_freq, *wsjtx_time, *wsjtx_sync));
        }
    }

    let match_percentage = 100.0 * matched_count as f32 / wsjtx_candidates.len() as f32;
    println!("\nMatched {}/{} WSJT-X candidates ({:.1}%)",
             matched_count, wsjtx_candidates.len(), match_percentage);

    if !mismatched.is_empty() {
        println!("\nMismatched WSJT-X candidates (freq, time, sync):");
        for (freq, time, sync) in mismatched.iter().take(10) {
            println!("  WSJTX: {:.1} Hz, {:.3} s, sync={:.3}", freq, time, sync);

            // Show closest RustyFt8 candidate for comparison
            if let Some(closest) = rust_candidates.iter().min_by_key(|rc| {
                ((rc.frequency - freq).powi(2) + (rc.time_offset - time).powi(2) * 100.0) as i32
            }) {
                println!("  Rust:  {:.1} Hz, {:.3} s, sync={:.3} (Δf={:.1}, Δt={:.3}, Δsync={:.3})",
                         closest.frequency, closest.time_offset, closest.sync_power,
                         closest.frequency - freq, closest.time_offset - time, closest.sync_power - sync);
            }
        }
        if mismatched.len() > 10 {
            println!("  ... and {} more", mismatched.len() - 10);
        }

        // Also show first 10 RustyFt8 candidates for comparison
        println!("\nFirst 10 RustyFt8 candidates:");
        for (i, rc) in rust_candidates.iter().take(10).enumerate() {
            println!("  {}: {:.1} Hz, {:.3} s, sync={:.3}",
                     i+1, rc.frequency, rc.time_offset, rc.sync_power);
        }
    }

    // STRICT REQUIREMENT: RustyFt8 should match WSJT-X sync8 output closely
    //
    // This test documents that RustyFt8's coarse sync implementation differs significantly
    // from the WSJT-X reference implementation. Key differences:
    //
    // 1. Sync power values differ by ~100x (WSJT-X: 237.8, RustyFt8: 2.5 for same signal)
    // 2. Candidate ordering is completely different due to different scoring
    // 3. Frequencies match reasonably well (within ~3 Hz)
    // 4. Time offsets have larger discrepancies
    //
    // TODO: Fix sync power calculation to match WSJT-X's sync8.f90:
    //   - Review normalization of sync correlation values
    //   - Check if we're missing the baseline noise calculation
    //   - Verify Costas array correlation implementation matches exactly
    //   - Ensure sync2d computation matches WSJT-X line-by-line
    //
    // Expected: >= 95% of candidates should match within strict tolerances

    const MIN_MATCH_PERCENTAGE: f32 = 95.0;

    assert!(match_percentage >= MIN_MATCH_PERCENTAGE,
            "\n❌ FAILED: RustyFt8 coarse_sync does not match WSJT-X reference implementation\n\
             \n\
             Matched only {:.1}% of WSJT-X candidates (required: {:.0}%)\n\
             \n\
             This indicates fundamental differences in sync scoring between RustyFt8 and WSJT-X.\n\
             Both found {} candidates, but:\n\
             - Sync power values differ by ~100x\n\
             - Candidate ordering is completely different\n\
             - Only {} out of {} candidates matched within strict tolerances\n\
             \n\
             See test output above for detailed comparison of mismatched candidates.\n\
             Review WSJT-X's sync8.f90 and compare with RustyFt8's coarse sync implementation.",
            match_percentage, MIN_MATCH_PERCENTAGE,
            wsjtx_candidates.len(),
            matched_count, wsjtx_candidates.len());

    println!("\n✅ SUCCESS: RustyFt8 coarse_sync matches WSJT-X sync8 reference ({:.1}% agreement)",
             match_percentage);
}
