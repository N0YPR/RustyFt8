//! Compare synthetic vs real signal for "K1BZM EA3GP -09"

use rustyft8::sync::{coarse_sync, fine_sync, extract_symbols_all_llr};

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn compare_synthetic_vs_real() {
    let test_cases = [
        ("Synthetic (clean)", "tests/test_data/synthetic_k1bzm.wav", 2695.0),
        ("Synthetic (with interference)", "tests/test_data/mixed_interference.wav", 2695.0),
        ("Real recording", "tests/test_data/210703_133430.wav", 2696.9),
    ];

    let expected_codeword: Vec<u8> = vec![
        0,0,0,0,1,0,0,1,1,0,1,1,1,1,1,0,0,0,1,1,1,0,1,0,0,0,0,0,0,0,1,1,
        0,1,1,0,1,0,1,0,0,0,1,0,1,0,1,1,0,0,0,1,0,0,1,0,0,0,0,1,1,1,1,1,
        1,0,1,0,1,0,1,0,1,0,0,0,1,
        0,1,1,1,1,0,0,1,0,0,1,0,0,1,
        1,1,1,1,1,1,0,1,0,0,1,1,1,1,0,1,1,1,0,0,0,0,1,0,1,0,0,0,0,1,1,1,
        0,0,0,0,0,1,0,1,0,0,0,1,0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,1,1,0,0,1,
        1,1,0,0,1,1,0,0,1,0,0,1,1,1,1,0,0,0,0,
    ];

    println!("\n=== Comparing Synthetic vs Real Signal ===\n");

    for (label, file_path, target_freq) in &test_cases {
        println!("--- {} ---", label);

        let signal = read_wav_file_raw(file_path)
            .expect(&format!("Failed to read {}", file_path));
        let signal = normalize_signal_length(signal);

        let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
            .expect("coarse_sync failed");

        // Find candidate near target frequency
        let mut found = false;
        for cand in &candidates {
            if (cand.frequency - target_freq).abs() < 2.0 {
                println!("Coarse: freq={:.1} Hz, time={:.3}s, sync={:.3}",
                         cand.frequency, cand.time_offset, cand.sync_power);

                let refined = match fine_sync(&signal, cand) {
                    Ok(r) => r,
                    Err(e) => {
                        println!("Fine sync failed: {}\n", e);
                        continue;
                    }
                };

                println!("Refined: freq={:.1} Hz, time={:.3}s",
                         refined.frequency, refined.time_offset);

                // Extract symbols
                let mut llra = vec![0.0f32; 174];
                let mut llrb = vec![0.0f32; 174];
                let mut llrc = vec![0.0f32; 174];
                let mut llrd = vec![0.0f32; 174];
                let mut s8 = [[0.0f32; 79]; 8];

                if extract_symbols_all_llr(&signal, &refined, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8).is_err() {
                    println!("Symbol extraction failed\n");
                    continue;
                }

                // Analyze quality
                let mean_abs: f32 = llra.iter().map(|&x| x.abs()).sum::<f32>() / 174.0;
                let max_llr = llra.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
                let min_llr = llra.iter().fold(f32::INFINITY, |a, &b| a.min(b));

                // Count bit errors
                let mut hard_decisions = vec![0u8; 174];
                for i in 0..174 {
                    hard_decisions[i] = if llra[i] > 0.0 { 1 } else { 0 };
                }

                let bit_errors: usize = expected_codeword.iter().zip(hard_decisions.iter())
                    .filter(|(exp, got)| exp != got)
                    .count();

                println!("LLRs: mean_abs={:.2}, range=[{:.2}, {:.2}]", mean_abs, min_llr, max_llr);
                println!("Bit errors: {}/174 ({:.1}%)", bit_errors, 100.0 * bit_errors as f32 / 174.0);

                // Show error positions for real signal
                if bit_errors > 0 {
                    let mut error_positions = Vec::new();
                    for i in 0..174 {
                        if expected_codeword[i] != hard_decisions[i] {
                            error_positions.push(i);
                        }
                    }
                    print!("Error positions: ");
                    for (idx, pos) in error_positions.iter().take(25).enumerate() {
                        print!("{}", pos);
                        if idx < error_positions.len().min(25) - 1 {
                            print!(",");
                        }
                    }
                    println!();
                }

                println!();
                found = true;
                break;
            }
        }

        if !found {
            println!("No candidate found near {:.0} Hz\n", target_freq);
        }
    }

    println!("=== Summary ===");
    println!("If synthetic signal has 0-2 errors but real has 20+:");
    println!("  → Algorithm is correct, real recording has interference or timing issues");
    println!("If both have similar high error rates:");
    println!("  → Fundamental algorithm bug");
}
