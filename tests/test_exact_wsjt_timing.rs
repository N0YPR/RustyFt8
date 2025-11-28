//! Test extraction at WSJT-X's exact reported time offset

use rustyft8::sync::{coarse_sync, extract_symbols_all_llr, Candidate};
use rustyft8::ldpc;
use bitvec::prelude::*;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file_raw, normalize_signal_length};

#[test]
#[ignore]
fn test_wsjt_x_exact_timing() {
    let signal = read_wav_file_raw("tests/test_data/210703_133430.wav")
        .expect("Failed to read WAV");
    let signal = normalize_signal_length(signal);

    let candidates = coarse_sync(&signal, 200.0, 4000.0, 1.0, 200)
        .expect("coarse_sync failed");

    println!("\n=== Testing WSJT-X Exact Timing for K1BZM EA3GP -09 ===\n");

    // Find 2695 Hz candidate
    for cand in &candidates {
        if (cand.frequency - 2696.9).abs() < 1.0 {
            println!("Found candidate: freq={:.1} Hz, time={:.3}s, sync={:.3}",
                     cand.frequency, cand.time_offset, cand.sync_power);

            // WSJT-X reports: freq=2695 Hz, DT=-0.1s
            // DT=-0.1s means 0.5s - 0.1s = 0.4s absolute time
            let wsjt_frequencies = [2695.0, 2695.4, 2696.9];  // Test WSJT-X exact freq, our refined, and coarse
            let wsjt_times = [0.4, 0.375, 0.38, 0.39, 0.41];  // WSJT-X time and nearby

            let expected_codeword: Vec<u8> = vec![
                0,0,0,0,1,0,0,1,1,0,1,1,1,1,1,0,0,0,1,1,1,0,1,0,0,0,0,0,0,0,1,1,
                0,1,1,0,1,0,1,0,0,0,1,0,1,0,1,1,0,0,0,1,0,0,1,0,0,0,0,1,1,1,1,1,
                1,0,1,0,1,0,1,0,1,0,0,0,1,
                0,1,1,1,1,0,0,1,0,0,1,0,0,1,
                1,1,1,1,1,1,0,1,0,0,1,1,1,1,0,1,1,1,0,0,0,0,1,0,1,0,0,0,0,1,1,1,
                0,0,0,0,0,1,0,1,0,0,0,1,0,1,1,1,0,0,0,1,0,0,0,0,0,0,0,1,1,0,0,1,
                1,1,0,0,1,1,0,0,1,0,0,1,1,1,1,0,0,0,0,
            ];

            let mut best_errors = 174;
            let mut best_freq = 0.0;
            let mut best_time = 0.0;
            let mut best_decoded = None;

            for &freq in &wsjt_frequencies {
                for &time in &wsjt_times {
                    let test_cand = Candidate {
                        frequency: freq,
                        time_offset: time,
                        sync_power: cand.sync_power,
                        baseline_noise: cand.baseline_noise,
                    };

                    let mut llra = vec![0.0f32; 174];
                    let mut llrb = vec![0.0f32; 174];
                    let mut llrc = vec![0.0f32; 174];
                    let mut llrd = vec![0.0f32; 174];
                    let mut s8 = [[0.0f32; 79]; 8];

                    if extract_symbols_all_llr(&signal, &test_cand, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8).is_err() {
                        continue;
                    }

                    // Count bit errors
                    let mut hard_decisions = vec![0u8; 174];
                    for i in 0..174 {
                        hard_decisions[i] = if llra[i] > 0.0 { 1 } else { 0 };
                    }

                    let bit_errors: usize = expected_codeword.iter().zip(hard_decisions.iter())
                        .filter(|(exp, got)| exp != got)
                        .count();

                    if bit_errors < best_errors {
                        best_errors = bit_errors;
                        best_freq = freq;
                        best_time = time;

                        // Try LDPC decoding
                        let methods = [
                            ("llra", &llra),
                            ("llrb", &llrb),
                            ("llrc", &llrc),
                            ("llrd", &llrd),
                        ];

                        for (name, llr) in &methods {
                            if let Some((bits, iters, _nharderrors)) = ldpc::decode_hybrid(llr, ldpc::DecodeDepth::BpOnly) {
                                let info_bits: BitVec<u8, Msb0> = bits.iter().take(77).collect();
                                if let Ok(msg) = rustyft8::decode(&info_bits, None) {
                                    best_decoded = Some((name.to_string(), msg, iters));
                                    break;
                                }
                            }
                        }
                    }

                    println!("  freq={:.1} Hz, time={:.3}s: {} bit errors ({:.1}%)",
                             freq, time, bit_errors, 100.0 * bit_errors as f32 / 174.0);

                    if let Some((method, msg, iters)) = &best_decoded {
                        if best_freq == freq && best_time == time {
                            println!("    ✓ DECODED with {}: \"{}\" (iters={})", method, msg, iters);
                        }
                    }
                }
            }

            println!("\n✓ Best result:");
            println!("  Frequency: {:.1} Hz", best_freq);
            println!("  Time: {:.3}s (WSJT-X uses 0.4s)", best_time);
            println!("  Bit errors: {}/174 ({:.1}%)", best_errors, 100.0 * best_errors as f32 / 174.0);

            if let Some((method, msg, iters)) = best_decoded {
                println!("  Decoded: \"{}\" using {} (iters={})", msg, method, iters);
                if msg == "K1BZM EA3GP -09" {
                    println!("  ✅ SUCCESS! Got correct message!");
                } else {
                    println!("  ❌ WRONG! Expected \"K1BZM EA3GP -09\" but got \"{}\"", msg);
                }
            } else {
                println!("  ❌ LDPC failed to converge");
            }

            break;
        }
    }
}
