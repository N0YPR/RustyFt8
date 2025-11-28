//! Compare a WORKING decode vs a FAILING decode to understand the difference

use rustyft8::sync::{coarse_sync, fine_sync, extract_symbols_all_llr};
use rustyft8::ldpc;
use bitvec::prelude::*;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn compare_working_vs_failing() {
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("\n=== Comparing WORKING vs FAILING messages ===\n");

    // Test messages with their expected codewords
    let test_cases = vec![
        (
            "W1FC F5BZB -08",  // WORKS - decoded successfully
            2571.0,
            vec![
                // 77 source + 14 CRC + 83 parity = 174 bits
                // Get from: ./wsjtx/.../ft8code "W1FC F5BZB -08"
                // Source: 01101000111100011011000000001100010000110011101101100110100011000001101100011
                // CRC: 00101001000001
                // Parity: 11000111111101010100011011000100011001011110011001100011000100000100010110000000111
                0,1,1,0,1,0,0,0,1,1,1,1,0,0,0,1,1,0,1,1,0,0,0,0,0,0,0,0,1,1,0,0,
                0,1,0,0,0,0,1,1,0,0,1,1,1,0,1,1,0,1,1,0,0,1,1,0,1,0,0,0,1,1,0,0,
                0,0,0,1,1,0,1,1,0,0,0,1,1,
                0,0,1,0,1,0,0,1,0,0,0,0,0,1,
                1,1,0,0,0,1,1,1,1,1,1,1,0,1,0,1,0,1,0,0,0,1,1,0,1,1,0,0,0,1,0,0,
                0,1,1,0,0,1,0,1,1,1,1,0,0,1,1,0,0,1,1,0,0,0,1,1,0,0,0,1,0,0,0,0,
                0,1,0,0,0,1,0,1,1,0,0,0,0,0,0,0,1,1,1,
            ]
        ),
        (
            "K1BZM EA3GP -09",  // FAILS - 21 bit errors
            2695.0,
            vec![
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
            ]
        ),
    ];

    for (msg_text, target_freq, expected_codeword) in test_cases {
        println!("=== {} @ {:.0} Hz ===", msg_text, target_freq);

        // Find candidate
        let mut found_cand = None;
        for cand in &candidates {
            if (cand.frequency - target_freq).abs() < 5.0 {
                found_cand = Some(cand.clone());
                break;
            }
        }

        let cand = match found_cand {
            Some(c) => c,
            None => {
                println!("  ✗ No candidate found near {:.0} Hz\n", target_freq);
                continue;
            }
        };

        println!("  Coarse: {:.1} Hz, time={:.3}s, sync={:.3}",
                 cand.frequency, cand.time_offset, cand.sync_power);

        // Fine sync
        let refined = match fine_sync(&signal, &cand) {
            Ok(r) => r,
            Err(e) => {
                println!("  ✗ Fine sync failed: {}\n", e);
                continue;
            }
        };

        println!("  Refined: {:.1} Hz, time={:.3}s",
                 refined.frequency, refined.time_offset);

        // Extract symbols
        let mut llra = vec![0.0f32; 174];
        let mut llrb = vec![0.0f32; 174];
        let mut llrc = vec![0.0f32; 174];
        let mut llrd = vec![0.0f32; 174];
        let mut s8 = [[0.0f32; 79]; 8];

        if extract_symbols_all_llr(&signal, &refined, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8).is_err() {
            println!("  ✗ Symbol extraction failed\n");
            continue;
        }

        // Analyze LLRs
        let mean_abs: f32 = llra.iter().map(|&x| x.abs()).sum::<f32>() / 174.0;
        let max = llra.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let near_zero = llra.iter().filter(|&&x| x.abs() < 0.5).count();

        // Hard decisions
        let mut hard_decisions = vec![0u8; 174];
        for i in 0..174 {
            hard_decisions[i] = if llra[i] > 0.0 { 1 } else { 0 };
        }

        let bit_errors: usize = expected_codeword.iter().zip(hard_decisions.iter())
            .filter(|(exp, got)| exp != got)
            .count();

        println!("  LLRs: mean_abs={:.2}, max={:.2}, near_zero={}/174", mean_abs, max, near_zero);
        println!("  Bit errors: {}/174 ({:.1}%)", bit_errors, 100.0 * bit_errors as f32 / 174.0);

        // Try LDPC decode
        let decode_result = ldpc::decode_hybrid(&llra, ldpc::DecodeDepth::BpOnly);
        match decode_result {
            Some((bits, iters)) => {
                let info_bits: BitVec<u8, Msb0> = bits.iter().take(77).collect();
                match rustyft8::decode(&info_bits, None) {
                    Ok(decoded_msg) => {
                        println!("  ✓ DECODED with BP (iters={}): \"{}\"", iters, decoded_msg);
                    }
                    Err(e) => {
                        println!("  ! BP converged but unpack failed: {}", e);
                    }
                }
            }
            None => {
                println!("  ✗ BP failed to converge");
            }
        }

        println!();
    }
}
