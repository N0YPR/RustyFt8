//! Integration tests using real FT8 recordings
//!
//! Tests the decoder against actual FT8 recordings to validate real-world performance.
//! Reference recordings are compared against WSJT-X output for validation.

use rustyft8::{decode_ft8, DecoderConfig, DecodedMessage};

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file, normalize_signal_length};

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_real_ft8_recording_210703_133430() {
    // Run this specific test with:
    // cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored --nocapture
    //
    // This test validates RustyFt8 against all 22 messages that WSJT-X decodes from this recording.
    // WSJT-X decodes these using (SNR range: 16 to -24 dB):
    // - Pure LDPC for 9 strong signals (SNR: -16 to 16 dB)
    // - AP with callsign hash table for 10 weak signals (requires mycall/hiscall or hash)
    // - OSD (Ordered Statistics Decoding) for 3 extremely weak signals (SNR <= -20 dB)
    //
    // RustyFt8 must implement to pass:
    // - Pure LDPC (already implemented) ✓
    // - AP decoding with callsign hash table (needed for 10 messages)
    // - OSD for extremely weak signals (needed for 3 messages)

    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV file");

    println!("Read {} samples from {}", signal.len(), wav_path);

    let signal_15s = normalize_signal_length(signal);
    // Increase decode_top_n and lower sync_threshold to find weaker signals
    // AP Type 1 (CQ pattern) enabled without callsign configuration (like WSJT-X)
    // let config = DecoderConfig {
    //     decode_top_n: 150,  // Attempt all found candidates
    //     sync_threshold: 0.4,  // Lower from 0.5 to find more candidates
    //     max_candidates: 150,  // Increase to find more weaker signals
    //     enable_ap: true,     // AP enabled by default (Type 1 works without callsigns)
    //     // No mycall/hiscall configured - fair comparison with WSJT-X
    //     ..DecoderConfig::default()
    // };
    let config = DecoderConfig::default();

    let mut decoded_messages: Vec<DecodedMessage> = Vec::new();
    // Use single-pass decoding (multi-pass creates false positives from subtraction artifacts)
    let count = decode_ft8(&signal_15s, &config, |msg| {
        println!("Decoded: {} @ {:.1} Hz, DT={:.2}s, SNR={} dB, sync={:.2}, LDPC iters={}, LLR scale={:.1}, nsym={}",
            msg.message, msg.frequency, msg.time_offset, msg.snr_db,
            msg.sync_power, msg.ldpc_iterations, msg.llr_scale, msg.nsym);
        decoded_messages.push(msg);
        true
    }).expect("Decode failed");

    println!("\nTotal decoded: {} messages", count);
    println!("WSJT-X reference: 22 messages");

    // All 22 messages that WSJT-X decodes from this recording
    // These include messages decoded with various techniques:
    // - Pure LDPC for strong signals (9 messages)
    // - AP with callsign hash table for weak signals (10 messages)
    // - Extremely weak signals requiring OSD (3 messages, SNR <= -20 dB)
    let expected_messages = vec![
        // Pure LDPC baseline (9 messages)
        "W1FC F5BZB -08",           // SNR: 16 dB (pure LDPC)
        "WM3PEN EA6VQ -09",         // SNR: 12 dB (pure LDPC)
        "CQ F5RXL IN94",            // SNR: -2 dB (pure LDPC)
        "K1JT HA0DU KN07",          // SNR: -14 dB (pure LDPC)
        "N1JFU EA6EE R-07",         // SNR: -12 dB (pure LDPC)
        "K1JT EA3AGB -15",          // SNR: -16 dB (pure LDPC)
        "W1DIG SV9CVY -14",         // SNR: -7 dB (pure LDPC)
        "W0RSJ EA3BMU RR73",        // SNR: -16 dB (pure LDPC)
        "XE2X HA2NP RR73",          // SNR: -11 dB (pure LDPC)

        // Messages requiring AP with callsign hash table (10 messages)
        "N1PJT HB9CQK -10",         // Requires AP with N1PJT or HB9CQK
        "KD2UGC F6GCP R-23",        // Requires AP with KD2UGC or F6GCP
        "A92EE F5PSR -14",          // Requires AP with A92EE or F5PSR
        "K1BZM EA3GP -09",          // Requires AP with K1BZM and EA3GP
        "K1BZM EA3CJ JN01",         // Requires AP with K1BZM and EA3CJ
        "WA2FZW DL5AXX RR73",       // Requires AP with WA2FZW or DL5AXX
        "N1API HA6FQ -23",          // Requires AP with N1API or HA6FQ
        "N1API F2VX 73",            // Requires AP with N1API or F2VX
        "CQ DX DL8YHR JO41",        // Weak CQ, needs AP Type 1 or better tuning
        "CQ EA2BFM IN83",           // Weak CQ, needs AP Type 1 or better tuning

        // Extremely weak signals requiring OSD (3 messages, SNR <= -20 dB)
        "K1JT HA5WA 73",            // SNR: -20 dB (requires OSD)
        "K1BZM DK8NE -10",          // SNR: -20 dB (requires OSD)
        "TU; 7N9RST EI8TRF 589 5732", // SNR: -24 dB (requires OSD)
    ];

    // Verify we decoded at least some messages
    assert!(!decoded_messages.is_empty(), "Should decode at least one message from real recording");

    // Verify all decoded messages are valid (non-empty, reasonable parameters)
    for msg in &decoded_messages {
        assert!(!msg.message.is_empty(), "Decoded message should not be empty");
        assert!(msg.frequency > 0.0 && msg.frequency < 4000.0,
            "Frequency {:.1} Hz should be in valid FT8 range", msg.frequency);
        assert!(msg.snr_db >= -25 && msg.snr_db <= 30,
            "SNR {} dB should be in reasonable range", msg.snr_db);
    }

    // Check that we decoded the expected strong signals
    let decoded_texts: Vec<String> = decoded_messages.iter()
        .map(|m| m.message.clone())
        .collect();

    let mut missing = Vec::new();
    for expected in &expected_messages {
        if !decoded_texts.contains(&expected.to_string()) {
            missing.push(*expected);
        }
    }

    if !missing.is_empty() {
        eprintln!("\n❌ Missing expected messages ({} of {}):", missing.len(), expected_messages.len());
        for msg in &missing {
            eprintln!("  - {}", msg);
        }
        eprintln!("\nDecoded messages ({}):", decoded_texts.len());
        for msg in &decoded_texts {
            eprintln!("  - {}", msg);
        }
        panic!("Failed to decode {} of {} expected WSJT-X messages", missing.len(), expected_messages.len());
    }

    // Report false positives (messages not in WSJT-X output at all)
    let false_positives: Vec<_> = decoded_texts.iter()
        .filter(|msg| !expected_messages.contains(&msg.as_str()))
        .collect();

    if !false_positives.is_empty() {
        println!("\nNote: {} additional message(s) decoded (not in WSJT-X output):",
            false_positives.len());
        for fp in &false_positives {
            println!("  - {}", fp);
        }
    }

    println!("\n✓ Successfully decoded all {} expected WSJT-X messages!", expected_messages.len());
    println!("  Total decoded: {} messages ({} expected + {} additional)",
        count, expected_messages.len(), false_positives.len());
    println!("\n  WSJT-X: 22 messages (9 pure LDPC + 10 AP with callsign hash + 3 OSD)");
    println!("  RustyFt8: {} messages", count);
}

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_frequency_range_filtering() {
    // Test that freq_min and freq_max actually filter signals by frequency
    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV file");

    let signal_15s = normalize_signal_length(signal);

    // First, decode with full frequency range to see what's there
    let full_config = DecoderConfig::default();
    let mut all_messages: Vec<DecodedMessage> = Vec::new();
    decode_ft8(&signal_15s, &full_config, |msg| {
        all_messages.push(msg);
        true
    }).expect("Decode failed");

    println!("Full range decoded {} messages", all_messages.len());
    for msg in &all_messages {
        println!("  {} @ {:.1} Hz", msg.message, msg.frequency);
    }

    // Find a message in the middle of the range to use as reference
    if all_messages.is_empty() {
        panic!("No messages decoded in full range - can't test filtering");
    }

    // Sort by frequency
    let mut sorted = all_messages.clone();
    sorted.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());

    // Use the median frequency as a split point
    let median_idx = sorted.len() / 2;
    let split_freq = sorted[median_idx].frequency;

    println!("\nSplit frequency: {:.1} Hz", split_freq);

    // Test freq_max: only decode signals below split frequency
    let low_config = DecoderConfig {
        freq_max: split_freq - 50.0, // Leave some margin
        ..DecoderConfig::default()
    };

    let mut low_messages: Vec<DecodedMessage> = Vec::new();
    decode_ft8(&signal_15s, &low_config, |msg| {
        low_messages.push(msg);
        true
    }).expect("Decode failed");

    println!("\nLow range (max={:.1} Hz) decoded {} messages", low_config.freq_max, low_messages.len());
    for msg in &low_messages {
        println!("  {} @ {:.1} Hz", msg.message, msg.frequency);
    }

    // Verify all decoded messages are below freq_max
    for msg in &low_messages {
        assert!(msg.frequency <= low_config.freq_max,
            "Message at {:.1} Hz exceeds freq_max of {:.1} Hz",
            msg.frequency, low_config.freq_max);
    }

    // Test freq_min: only decode signals above split frequency
    let high_config = DecoderConfig {
        freq_min: split_freq + 50.0, // Leave some margin
        ..DecoderConfig::default()
    };

    let mut high_messages: Vec<DecodedMessage> = Vec::new();
    decode_ft8(&signal_15s, &high_config, |msg| {
        high_messages.push(msg);
        true
    }).expect("Decode failed");

    println!("\nHigh range (min={:.1} Hz) decoded {} messages", high_config.freq_min, high_messages.len());
    for msg in &high_messages {
        println!("  {} @ {:.1} Hz", msg.message, msg.frequency);
    }

    // Verify all decoded messages are above freq_min
    for msg in &high_messages {
        assert!(msg.frequency >= high_config.freq_min,
            "Message at {:.1} Hz is below freq_min of {:.1} Hz",
            msg.frequency, high_config.freq_min);
    }

    // Verify we decoded fewer messages in each filtered range than full range
    assert!(low_messages.len() < all_messages.len(),
        "Filtered low range should decode fewer messages than full range");
    assert!(high_messages.len() < all_messages.len(),
        "Filtered high range should decode fewer messages than full range");

    println!("\n✓ Frequency filtering validated:");
    println!("  Full range: {} messages", all_messages.len());
    println!("  Low range:  {} messages (freq < {:.1} Hz)", low_messages.len(), low_config.freq_max);
    println!("  High range: {} messages (freq > {:.1} Hz)", high_messages.len(), high_config.freq_min);
}

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_wav_reader_format_validation() {
    // Verify WAV file reading and format validation works correctly
    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV file");

    let signal_15s = normalize_signal_length(signal);

    // Check normalized length
    assert_eq!(signal_15s.len(), 15 * 12000,
        "Signal should be exactly 15 seconds (180,000 samples)");

    // Check signal contains actual data (not all zeros)
    let sum: f32 = signal_15s.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Signal should contain non-zero samples");

    // Check amplitude is properly normalized to [-1.0, 1.0]
    let max_amp = signal_15s.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    assert!(max_amp > 0.0 && max_amp <= 1.0,
        "Signal amplitude {:.3} should be in range (0.0, 1.0]", max_amp);

    println!("✓ WAV file format validated: {} samples, max amplitude: {:.3}",
        signal_15s.len(), max_amp);
}

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_wsjtx_minus15db_signal() {
    // Test decoding a -15 dB signal generated by WSJT-X ft8sim
    // This validates SNR calculation and weak signal decoding
    let wav_path = "tests/test_data/wsjtx_minus15db.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WSJT-X test signal");

    println!("Read {} samples from WSJT-X -15 dB test signal", signal.len());

    let signal_15s = normalize_signal_length(signal);
    let config = DecoderConfig::default();

    let mut decoded_messages: Vec<DecodedMessage> = Vec::new();
    let count = decode_ft8(&signal_15s, &config, |msg| {
        println!("Decoded: {} @ {:.1} Hz, DT={:.2}s, SNR={} dB, sync={:.2}",
            msg.message, msg.frequency, msg.time_offset, msg.snr_db, msg.sync_power);
        decoded_messages.push(msg);
        true
    }).expect("Decode failed");

    println!("\nTotal decoded: {} messages", count);
    println!("Expected: 'CQ W1ABC FN42' at SNR -15 dB (WSJT-X measured)");

    // Should decode the expected message
    assert!(count > 0, "Should decode at least one message");

    let expected = "CQ W1ABC FN42";
    let found_msg = decoded_messages.iter().find(|m| m.message == expected);
    assert!(found_msg.is_some(),
        "Should decode expected message: '{}'. Decoded: {:?}",
        expected,
        decoded_messages.iter().map(|m| &m.message).collect::<Vec<_>>());

    // Verify SNR is within reasonable range of WSJT-X's measurement
    let decoded = found_msg.unwrap();
    let snr_diff = (decoded.snr_db as f32 - (-15.0)).abs();

    println!("\n✓ Decoded '{}' at SNR {} dB", decoded.message, decoded.snr_db);
    println!("  WSJT-X reported: -15 dB");
    println!("  Difference: {:.1} dB", decoded.snr_db as f32 - (-15.0));

    // Allow ±5 dB tolerance (SNR calculation methods may differ slightly)
    assert!(snr_diff <= 5.0,
        "SNR {} dB should be within 5 dB of WSJT-X's -15 dB measurement (diff: {:.1} dB)",
        decoded.snr_db, snr_diff);
}
