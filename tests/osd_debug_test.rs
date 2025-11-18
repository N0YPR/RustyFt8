use rustyft8::{sync, DecoderConfig, ldpc, crc};
use hound;
use bitvec::prelude::*;

#[test]
#[ignore] // Debug test - run with: cargo test -- --ignored
fn debug_first_osd_failure() {
    let wav_path = "tests/test_data/210703_133430.wav";
    let mut reader = hound::WavReader::open(wav_path).expect("Failed to open WAV file");
    let samples: Vec<f32> = reader.samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    let config = DecoderConfig::default();

    let candidates = sync::coarse_sync(
        &samples,
        config.freq_min,
        config.freq_max,
        config.sync_threshold,
        config.max_candidates,
    ).expect("Coarse sync failed");

    // Find first BP failure
    for (idx, candidate) in candidates.iter().take(config.decode_top_n).enumerate() {
        let refined = match sync::fine_sync(&samples, candidate) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let mut llr = vec![0.0f32; 174];
        if sync::extract_symbols(&samples, &refined, 1, &mut llr).is_err() {
            continue;
        }

        // Try BP first
        let bp_result = ldpc::decode(&llr, 200);

        if bp_result.is_none() {
            println!("\n=== First BP Failure Found ===");
            println!("Candidate #{} @ {:.1} Hz", idx + 1, refined.frequency);

            // Print LLR statistics
            let mut llr_sorted: Vec<f32> = llr.iter().map(|&x| x.abs()).collect();
            llr_sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(core::cmp::Ordering::Equal));

            println!("\nLLR Statistics:");
            println!("  Min reliability (|LLR|): {:.3}", llr_sorted[173]);
            println!("  Max reliability (|LLR|): {:.3}", llr_sorted[0]);
            println!("  Median reliability: {:.3}", llr_sorted[87]);
            println!("  10th percentile: {:.3}", llr_sorted[16]);
            println!("  90th percentile: {:.3}", llr_sorted[156]);

            // Make hard decisions
            let mut hard_dec = bitvec![u8, Msb0; 0; 174];
            for i in 0..174 {
                hard_dec.set(i, llr[i] >= 0.0);
            }

            // Check if hard decisions pass CRC
            let msg91: BitVec<u8, Msb0> = hard_dec[0..91].to_bitvec();
            let hard_crc_ok = crc::crc14_check(&msg91);
            println!("\nHard decision CRC check: {}", if hard_crc_ok { "PASS" } else { "FAIL" });

            // Try to encode the hard decision message and see how many bits differ
            let mut encoded = bitvec![u8, Msb0; 0; 174];
            ldpc::encode(&msg91, &mut encoded);

            let mut hamming_dist = 0;
            for i in 0..174 {
                if hard_dec[i] != encoded[i] {
                    hamming_dist += 1;
                }
            }
            println!("Hamming distance to valid codeword: {}", hamming_dist);

            // Try OSD
            println!("\nTrying OSD order-0...");
            let osd_result = ldpc::osd_decode(&llr, 0);

            if osd_result.is_some() {
                println!("Result: SUCCESS");
            } else {
                println!("Result: FAILED");

                println!("\nTrying OSD order-1...");
                let osd_result = ldpc::osd_decode(&llr, 1);

                if osd_result.is_some() {
                    println!("Result: SUCCESS");
                } else {
                    println!("Result: FAILED");
                }
            }

            break;
        }
    }
}
