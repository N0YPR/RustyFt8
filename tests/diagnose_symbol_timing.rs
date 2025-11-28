//! Diagnostic for symbol timing errors in K1BZM EA3GP -09 at 2695 Hz

use rustyft8::sync::{coarse_sync, fine_sync, extract_symbols_all_llr};

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn diagnose_symbol_timing_offset() {
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    // Run coarse sync
    let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("\n=== Symbol Timing Diagnostic for 2695 Hz ===\n");

    // Find the 2695 Hz candidate
    for cand in &candidates {
        if (cand.frequency - 2696.9).abs() < 1.0 {
            println!("Coarse: freq={:.1} Hz, time={:.3}s, sync={:.3}",
                     cand.frequency, cand.time_offset, cand.sync_power);

            // Fine sync
            let refined = fine_sync(&signal, cand).expect("fine_sync failed");
            println!("Refined: freq={:.1} Hz, time={:.3}s\n",
                     refined.frequency, refined.time_offset);

            // Expected codeword for "K1BZM EA3GP -09"
            let expected_codeword: Vec<u8> = vec![
                // 77 source bits
                0,0,0,0,1,0,0,1,1,0,1,1,1,1,1,0,0,0,1,1,1,0,1,0,0,0,0,0,0,0,1,1,
                0,1,1,0,1,0,1,0,0,0,1,0,1,0,1,1,0,0,0,1,0,0,1,0,0,0,0,1,1,1,1,1,
                1,0,1,0,1,0,1,0,1,0,0,0,1,
                // 14 CRC bits
                0,1,1,1,1,0,0,1,0,0,1,0,0,1,
                // 83 parity bits
                1,1,1,1,1,1,0,1,0,0,1,1,1,1,0,1,1,1,0,0,0,0,1,0,1,0,0,0,0,1,1,1,
                0,0,0,0,0,1,0,1,0,0,0,1,0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,1,1,0,0,1,
                1,1,0,0,1,1,0,0,1,0,0,1,1,1,1,0,0,0,0,
            ];

            println!("Testing symbol timing offsets (adjusting time by fractions of a symbol):");
            println!("Symbol duration: 160ms = 30 samples at 187.5 Hz\n");

            // Test timing offsets from -15 to +15 samples (half symbol on each side)
            let mut best_bit_errors = 174;
            let mut best_offset_ms = 0.0;

            for offset_samples in -15..=15 {
                // Convert sample offset to time offset
                let sample_rate = 187.5; // Typical downsampled rate
                let time_offset_adjustment = offset_samples as f32 / sample_rate;

                // Create adjusted candidate
                let mut test_cand = refined.clone();
                test_cand.time_offset += time_offset_adjustment;

                // Extract symbols with this timing
                let mut llra = vec![0.0f32; 174];
                let mut llrb = vec![0.0f32; 174];
                let mut llrc = vec![0.0f32; 174];
                let mut llrd = vec![0.0f32; 174];
                let mut s8 = [[0.0f32; 79]; 8];

                if extract_symbols_all_llr(&signal, &test_cand, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8).is_err() {
                    continue;
                }

                // Make hard decisions
                let mut hard_decisions = vec![0u8; 174];
                for i in 0..174 {
                    hard_decisions[i] = if llra[i] > 0.0 { 1 } else { 0 };
                }

                // Count bit errors
                let bit_errors: usize = expected_codeword.iter().zip(hard_decisions.iter())
                    .filter(|(exp, got)| exp != got)
                    .count();

                // Track best result
                if bit_errors < best_bit_errors {
                    best_bit_errors = bit_errors;
                    best_offset_ms = time_offset_adjustment * 1000.0;
                }

                // Print results
                if offset_samples % 3 == 0 || bit_errors < 15 {
                    println!("  Offset {:+3} samples ({:+6.2} ms): {} bit errors ({:.1}%)",
                             offset_samples, time_offset_adjustment * 1000.0,
                             bit_errors, 100.0 * bit_errors as f32 / 174.0);
                }
            }

            println!("\n✓ Best timing: {:+.2} ms offset, {} bit errors ({:.1}%)",
                     best_offset_ms, best_bit_errors, 100.0 * best_bit_errors as f32 / 174.0);

            if best_bit_errors < 21 {
                println!("  → Symbol timing adjustment could improve decoding!");
                println!("  → Current: 21 errors, Optimal: {} errors", best_bit_errors);
            } else if best_bit_errors > 21 {
                println!("  → Symbol timing is already optimal");
                println!("  → Issue must be elsewhere (phase, frequency tracking, etc.)");
            } else {
                println!("  → Symbol timing adjustment doesn't help");
                println!("  → Need to investigate other causes");
            }

            break;
        }
    }
}
