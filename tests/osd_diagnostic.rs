use rustyft8::{sync, DecoderConfig, ldpc};
use hound;

#[test]
fn diagnose_osd_effectiveness() {
    let wav_path = "tests/test_data/210703_133430.wav";
    let mut reader = hound::WavReader::open(wav_path).expect("Failed to open WAV file");
    let samples: Vec<f32> = reader.samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    let config = DecoderConfig::default();

    println!("\n=== OSD Diagnostic ===\n");

    // Get candidates
    let candidates = sync::coarse_sync(
        &samples,
        config.freq_min,
        config.freq_max,
        config.sync_threshold,
        config.max_candidates,
    ).expect("Coarse sync failed");

    println!("Testing {} candidates\n", candidates.len().min(config.decode_top_n));

    let mut bp_success = 0;
    let mut bp_fail = 0;
    let mut osd_success = 0;
    let mut osd_fail = 0;

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

        if bp_result.is_some() {
            bp_success += 1;
            println!("  #{}: BP SUCCESS @ {:.1} Hz", idx + 1, refined.frequency);
        } else {
            bp_fail += 1;

            // Try OSD with order-1 (allows single bit flips)
            let osd_result = ldpc::osd_decode(&llr, 1);

            if osd_result.is_some() {
                osd_success += 1;
                println!("  #{}: BP failed, OSD SUCCESS @ {:.1} Hz", idx + 1, refined.frequency);
            } else {
                osd_fail += 1;
                if idx < 15 {  // Only print first 15 failures
                    println!("  #{}: BP failed, OSD failed @ {:.1} Hz", idx + 1, refined.frequency);
                }
            }
        }
    }

    println!("\n=== Summary ===");
    println!("BP succeeded:  {}", bp_success);
    println!("BP failed:     {}", bp_fail);
    println!("  - OSD recovered: {}", osd_success);
    println!("  - Still failed:  {}", osd_fail);
    println!("\nTotal decoded: {} (BP) + {} (OSD) = {}", bp_success, osd_success, bp_success + osd_success);
    println!("WSJT-X reference: 22");
}
