//! Diagnostic for K1BZM EA3GP -09 at 2695 Hz

use rustyft8::sync::{coarse_sync, fine_sync, extract_symbols_all_llr};
use rustyft8::ldpc;
use bitvec::prelude::*;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn diagnose_2695_hz_candidate() {
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    // Run coarse sync
    let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("\n=== Searching for 2695 Hz candidate ===\n");

    // Find the 2695 Hz candidate
    for (idx, cand) in candidates.iter().enumerate() {
        if (cand.frequency - 2696.9).abs() < 1.0 {
            println!("[{}] Coarse: freq={:.1} Hz, time={:.3}s, sync={:.3}",
                     idx, cand.frequency, cand.time_offset, cand.sync_power);

            // Fine sync
            let refined = fine_sync(&signal, cand).expect("fine_sync failed");
            println!("  Refined: freq={:.1} Hz, time={:.3}s\n",
                     refined.frequency, refined.time_offset);

            // Try extracting at COARSE time vs FINE time
            let time_offsets = vec![
                ("Coarse time", cand.time_offset),
                ("Fine time", refined.time_offset),
                ("Fine - 0.5s", refined.time_offset - 0.5),
                ("Fine + 0.5s", refined.time_offset + 0.5),
            ];

            for (label, time_offset) in time_offsets {
                println!("--- Trying {} ({:.3}s) ---", label, time_offset);

                // Create modified candidate with this time
                let mut test_cand = refined.clone();
                test_cand.time_offset = time_offset;

                // Extract LLRs
                let mut llra = vec![0.0f32; 174];
                let mut llrb = vec![0.0f32; 174];
                let mut llrc = vec![0.0f32; 174];
                let mut llrd = vec![0.0f32; 174];
                let mut s8 = [[0.0f32; 79]; 8];

                if extract_symbols_all_llr(&signal, &test_cand,
                                          &mut llra, &mut llrb, &mut llrc, &mut llrd,
                                          &mut s8).is_err() {
                    println!("  Symbol extraction failed!\n");
                    continue;
                }

                // Show LLR statistics
                let mean_abs: f32 = llra.iter().map(|&x| x.abs()).sum::<f32>() / 174.0;
                let max = llra.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
                let min = llra.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                println!("  LLRa: mean_abs={:.3}, range=[{:.2}, {:.2}]", mean_abs, min, max);

                // Show first 20 LLRs
                print!("  First 20 LLRs: ");
                for i in 0..20 {
                    print!("{:.2} ", llra[i]);
                }
                println!();

                // Count how many LLRs are close to zero (unreliable)
                let near_zero = llra.iter().filter(|&&x| x.abs() < 0.5).count();
                println!("  LLRs near zero (<0.5): {}/174 ({:.1}%)",
                         near_zero, 100.0 * near_zero as f32 / 174.0);

                // Compute hard decisions and compare with correct codeword
                // Expected message: "K1BZM EA3GP -09"
                // Expected 174-bit codeword from ft8code:
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

                // Make hard decisions from LLRs (positive → 1, negative → 0)
                let mut hard_decisions = vec![0u8; 174];
                for i in 0..174 {
                    hard_decisions[i] = if llra[i] > 0.0 { 1 } else { 0 };
                }

                // Count bit errors
                let bit_errors: usize = expected_codeword.iter().zip(hard_decisions.iter())
                    .filter(|(exp, got)| exp != got)
                    .count();

                println!("  Bit errors (hard decisions vs expected): {}/174 ({:.1}%)",
                         bit_errors, 100.0 * bit_errors as f32 / 174.0);

                // Show first 20 bits comparison
                print!("  First 20 bits - Expected: ");
                for i in 0..20 { print!("{}", expected_codeword[i]); }
                println!();
                print!("  First 20 bits - Got:      ");
                for i in 0..20 { print!("{}", hard_decisions[i]); }
                println!();

                // Show which bits are wrong and look for patterns
                let mut error_positions = Vec::new();
                for i in 0..174 {
                    if expected_codeword[i] != hard_decisions[i] {
                        error_positions.push(i);
                    }
                }
                print!("  Error positions: ");
                for (idx, pos) in error_positions.iter().take(21).enumerate() {
                    print!("{}", pos);
                    if idx < error_positions.len() - 1 && idx < 20 {
                        print!(",");
                    }
                }
                println!();

                // Try decoding with all 4 methods
                let methods = [
                    ("llra", &llra[..]),
                    ("llrb", &llrb[..]),
                    ("llrc", &llrc[..]),
                    ("llrd", &llrd[..]),
                ];

                let mut bp_attempts = 0;
                let mut bp_successes = 0;
                let mut osd_attempts = 0;
                let mut osd_successes = 0;

                // Try with NEGATED LLRs first (test for sign inversion bug)
                println!("  Testing NEGATED LLRs (sign inversion test):");
                for &scale in &[1.0, 1.5] {
                    let mut llr_neg: Vec<f32> = llra.iter().map(|&x| -x * scale).collect();
                    if let Some((bits, iters)) = ldpc::decode_hybrid(&llr_neg, ldpc::DecodeDepth::BpOsdHybrid) {
                        let info_bits: BitVec<u8, Msb0> = bits.iter().take(77).collect();
                        if let Ok(msg) = rustyft8::decode(&info_bits, None) {
                            println!("    ✓✓✓ DECODED WITH NEGATED LLRs [llra×-{}]: \"{}\" (iters={})",
                                     scale, msg, iters);
                        }
                    }
                }
                println!();

                for &(method_name, llr_base) in &methods {
                    for &scale in &[1.0, 1.5, 0.75, 2.0] {
                        let mut llr_scaled: Vec<f32> = llr_base.iter().map(|&x| x * scale).collect();

                        // Try BP only first
                        bp_attempts += 1;
                        match ldpc::decode_hybrid(&llr_scaled, ldpc::DecodeDepth::BpOnly) {
                            Some((bits, iters)) => {
                                bp_successes += 1;
                                let info_bits: BitVec<u8, Msb0> = bits.iter().take(77).collect();
                                match rustyft8::decode(&info_bits, None) {
                                    Ok(msg) => {
                                        println!("  ✓ BP DECODED [{}×{}]: \"{}\" (iters={})",
                                                 method_name, scale, msg, iters);
                                    }
                                    Err(e) => {
                                        println!("  ! BP converged but unpack failed [{}×{}]: {}",
                                                 method_name, scale, e);
                                    }
                                }
                            }
                            None => {}
                        }

                        // Try with OSD
                        osd_attempts += 1;
                        match ldpc::decode_hybrid(&llr_scaled, ldpc::DecodeDepth::BpOsdHybrid) {
                            Some((bits, iters)) => {
                                osd_successes += 1;
                                let info_bits: BitVec<u8, Msb0> = bits.iter().take(77).collect();
                                match rustyft8::decode(&info_bits, None) {
                                    Ok(msg) => {
                                        println!("  ✓ OSD DECODED [{}×{}]: \"{}\" (iters={})",
                                                 method_name, scale, msg, iters);
                                    }
                                    Err(e) => {
                                        println!("  ! OSD converged but unpack failed [{}×{}]: {}",
                                                 method_name, scale, e);
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                }

                println!("  LDPC Summary: BP {}/{} converged, OSD {}/{} converged",
                         bp_successes, bp_attempts, osd_successes, osd_attempts);
                println!();
            }

            break;
        }
    }
}
