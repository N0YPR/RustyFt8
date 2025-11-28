//! Test if frequency adjustment reduces bit errors for K1BZM EA3GP -09

use rustyft8::sync::{coarse_sync, fine_sync, extract_symbols_all_llr};

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn diagnose_frequency_error() {
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("\n=== Frequency Error Diagnostic for 2695 Hz ===\n");

    // Find the 2695 Hz candidate
    for cand in &candidates {
        if (cand.frequency - 2696.9).abs() < 1.0 {
            println!("Coarse: freq={:.1} Hz, time={:.3}s, sync={:.3}",
                     cand.frequency, cand.time_offset, cand.sync_power);

            let refined = fine_sync(&signal, cand).expect("fine_sync failed");
            println!("Fine sync: freq={:.1} Hz, time={:.3}s\n",
                     refined.frequency, refined.time_offset);

            // Expected codeword
            let expected_codeword: Vec<u8> = vec![
                0,0,0,0,1,0,0,1,1,0,1,1,1,1,1,0,0,0,1,1,1,0,1,0,0,0,0,0,0,0,1,1,
                0,1,1,0,1,0,1,0,0,0,1,0,1,0,1,1,0,0,0,1,0,0,1,0,0,0,0,1,1,1,1,1,
                1,0,1,0,1,0,1,0,1,0,0,0,1,
                0,1,1,1,1,0,0,1,0,0,1,0,0,1,
                1,1,1,1,1,1,0,1,0,0,1,1,1,1,0,1,1,1,0,0,0,0,1,0,1,0,0,0,0,1,1,1,
                0,0,0,0,0,1,0,1,0,0,0,1,0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,1,1,0,0,1,
                1,1,0,0,1,1,0,0,1,0,0,1,1,1,1,0,0,0,0,
            ];

            println!("Testing frequency offsets (refined={:.1} Hz):", refined.frequency);
            let mut best_bit_errors = 174;
            let mut best_freq_offset = 0.0;

            // Test frequency offsets from -2.0 Hz to +2.0 Hz
            for freq_offset_int in -20..=20 {
                let freq_offset = freq_offset_int as f32 * 0.1; // 0.1 Hz steps

                let mut test_cand = refined.clone();
                test_cand.frequency += freq_offset;

                // Extract symbols
                let mut llra = vec![0.0f32; 174];
                let mut llrb = vec![0.0f32; 174];
                let mut llrc = vec![0.0f32; 174];
                let mut llrd = vec![0.0f32; 174];
                let mut s8 = [[0.0f32; 79]; 8];

                if extract_symbols_all_llr(&signal, &test_cand, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8).is_err() {
                    continue;
                }

                // Hard decisions
                let mut hard_decisions = vec![0u8; 174];
                for i in 0..174 {
                    hard_decisions[i] = if llra[i] > 0.0 { 1 } else { 0 };
                }

                let bit_errors: usize = expected_codeword.iter().zip(hard_decisions.iter())
                    .filter(|(exp, got)| exp != got)
                    .count();

                if bit_errors < best_bit_errors {
                    best_bit_errors = bit_errors;
                    best_freq_offset = freq_offset;
                }

                // Print every 0.5 Hz or if significantly better
                if freq_offset_int % 5 == 0 || bit_errors < 15 {
                    println!("  {:+5.1} Hz: {} bit errors ({:.1}%)",
                             freq_offset, bit_errors, 100.0 * bit_errors as f32 / 174.0);
                }
            }

            println!("\n✓ Best frequency: {:.1} Hz ({:+.1} Hz offset), {} bit errors ({:.1}%)",
                     refined.frequency + best_freq_offset, best_freq_offset,
                     best_bit_errors, 100.0 * best_bit_errors as f32 / 174.0);

            if best_bit_errors < 21 {
                println!("  → Frequency adjustment helps!");
                println!("  → Current: 21 errors @ {:.1} Hz", refined.frequency);
                println!("  → Optimal: {} errors @ {:.1} Hz", best_bit_errors, refined.frequency + best_freq_offset);
                println!("  → Fine sync may need tighter frequency tolerance");
            } else if best_bit_errors == 21 {
                println!("  → Frequency is already optimal");
                println!("  → Issue must be timing/phase related");
            } else {
                println!("  → Frequency adjustment makes it worse!");
                println!("  → Current setting is best");
            }

            break;
        }
    }
}
