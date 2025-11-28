//! Test combined timing + frequency adjustment

use rustyft8::sync::{coarse_sync, fine_sync, extract_symbols_all_llr};

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn test_combined_timing_frequency_adjustment() {
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("\n=== Combined Timing + Frequency Adjustment ===\n");

    for cand in &candidates {
        if (cand.frequency - 2696.9).abs() < 1.0 {
            let refined = fine_sync(&signal, cand).expect("fine_sync failed");
            println!("Base: freq={:.1} Hz, time={:.3}s\n", refined.frequency, refined.time_offset);

            let expected_codeword: Vec<u8> = vec![
                0,0,0,0,1,0,0,1,1,0,1,1,1,1,1,0,0,0,1,1,1,0,1,0,0,0,0,0,0,0,1,1,
                0,1,1,0,1,0,1,0,0,0,1,0,1,0,1,1,0,0,0,1,0,0,1,0,0,0,0,1,1,1,1,1,
                1,0,1,0,1,0,1,0,1,0,0,0,1,0,1,1,1,1,0,0,1,0,0,1,0,0,1,
                1,1,1,1,1,1,0,1,0,0,1,1,1,1,0,1,1,1,0,0,0,0,1,0,1,0,0,0,0,1,1,1,
                0,0,0,0,0,1,0,1,0,0,0,1,0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,1,1,0,0,1,
                1,1,0,0,1,1,0,0,1,0,0,1,1,1,1,0,0,0,0,
            ];

            let mut best_errors = 174;
            let mut best_freq = 0.0;
            let mut best_time = 0.0;

            // Test grid: frequency ±1.0 Hz, timing ±80ms
            println!("Testing combined adjustments...");
            for freq_offset_int in -10..=10 {
                let freq_offset = freq_offset_int as f32 * 0.1;

                for time_offset_samples in vec![-15, -12, -9, -6, -3, 0, 3, 6, 9, 12, 15] {
                    let sample_rate = 187.5;
                    let time_offset = time_offset_samples as f32 / sample_rate;

                    let mut test_cand = refined.clone();
                    test_cand.frequency += freq_offset;
                    test_cand.time_offset += time_offset;

                    let mut llra = vec![0.0f32; 174];
                    let mut llrb = vec![0.0f32; 174];
                    let mut llrc = vec![0.0f32; 174];
                    let mut llrd = vec![0.0f32; 174];
                    let mut s8 = [[0.0f32; 79]; 8];

                    if extract_symbols_all_llr(&signal, &test_cand, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8).is_err() {
                        continue;
                    }

                    let mut hard_decisions = vec![0u8; 174];
                    for i in 0..174 {
                        hard_decisions[i] = if llra[i] > 0.0 { 1 } else { 0 };
                    }

                    let bit_errors: usize = expected_codeword.iter().zip(hard_decisions.iter())
                        .filter(|(exp, got)| exp != got)
                        .count();

                    if bit_errors < best_errors {
                        best_errors = bit_errors;
                        best_freq = freq_offset;
                        best_time = time_offset;
                        println!("  New best: freq {:+.1} Hz, time {:+.1} ms → {} errors",
                                 freq_offset, time_offset * 1000.0, bit_errors);
                    }
                }
            }

            println!("\n✓ Best combined adjustment:");
            println!("  Frequency: {:+.1} Hz (total={:.1} Hz)", best_freq, refined.frequency + best_freq);
            println!("  Timing: {:+.1} ms (total={:.3}s)", best_time * 1000.0, refined.time_offset + best_time);
            println!("  Bit errors: {} ({:.1}%)", best_errors, 100.0 * best_errors as f32 / 174.0);
            println!();
            println!("Comparison:");
            println!("  Baseline (no adjustment): 21 errors");
            println!("  Frequency only (-0.3 Hz): 19 errors");
            println!("  Timing only (+75 ms): 19 errors");
            println!("  Combined: {} errors", best_errors);

            if best_errors < 10 {
                println!("\n✅ SUCCESS! Under 10 errors - LDPC should converge!");
            } else if best_errors < 15 {
                println!("\n⚠️  Close! 10-15 errors - LDPC might converge with OSD");
            } else {
                println!("\n❌ Still too many errors (>15) - deeper issue remains");
            }

            break;
        }
    }
}
