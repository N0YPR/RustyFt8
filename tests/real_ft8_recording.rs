//! Integration tests using real FT8 recordings
//!
//! Tests the decoder against actual FT8 recordings to validate real-world performance.
//! Reference recordings are compared against WSJT-X output for validation.

use rustyft8::{decode_ft8, DecoderConfig, DecodedMessage};
use std::time::Instant;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file, normalize_signal_length};

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_real_ft8_recording_210703_133430() {
    // Validates RustyFt8 against WSJT-X using multipass decoding with signal subtraction.
    // WSJT-X decodes 22 messages from this recording (SNR range: 16 to -24 dB).
    //
    // Run with: cargo test --release --test real_ft8_recording test_real_ft8_recording_210703_133430 -- --ignored

    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path).expect("Failed to read WAV file");
    let signal_15s = normalize_signal_length(signal);
    let config = DecoderConfig::default();

    let mut decoded_messages: Vec<DecodedMessage> = Vec::new();
    let start_time = Instant::now();
    let _count = decode_ft8(&signal_15s, &config, |msg| {
        decoded_messages.push(msg);
        true
    }).expect("Decode failed");
    let decode_duration = start_time.elapsed();

    // Messages that MUST decode for the test to pass
    let required_messages = [
        "W1FC F5BZB -08",
        "WM3PEN EA6VQ -09",
        "CQ F5RXL IN94",
        "K1JT HA0DU KN07",
        "A92EE F5PSR -14",
        "N1JFU EA6EE R-07",
        "K1JT EA3AGB -15",
        "W1DIG SV9CVY -14",
        "W0RSJ EA3BMU RR73",
        "XE2X HA2NP RR73",
        "K1BZM EA3CJ JN01",
        "WA2FZW DL5AXX RR73",
        "N1PJT HB9CQK -10",
        "KD2UGC F6GCP R-23",
        "K1BZM EA3GP -09",
        "N1API HA6FQ -23",
        "CQ EA2BFM IN83",
    ];

    // Messages not yet decoded (tracking progress toward WSJT-X parity)
    let todo_messages = [
        "N1API F2VX 73",
        "CQ DX DL8YHR JO41",
        "K1JT HA5WA 73",
        "K1BZM DK8NE -10",
        "TU; 7N9RST EI8TRF 589 5732",
    ];

    let all_expected: Vec<&str> = required_messages.iter()
        .chain(todo_messages.iter())
        .cloned()
        .collect();

    // Verify decoded messages are valid
    for msg in &decoded_messages {
        assert!(!msg.message.is_empty(), "Decoded message should not be empty");
        assert!(msg.frequency > 0.0 && msg.frequency < 4000.0,
            "Frequency {:.1} Hz should be in valid FT8 range", msg.frequency);
        assert!(msg.snr_db >= -25 && msg.snr_db <= 30,
            "SNR {} dB should be in reasonable range", msg.snr_db);
    }

    let decoded_texts: Vec<String> = decoded_messages.iter()
        .map(|m| m.message.clone())
        .collect();

    let missing_required: Vec<_> = required_messages.iter()
        .filter(|msg| !decoded_texts.contains(&msg.to_string()))
        .collect();

    let todo_decoded: Vec<_> = todo_messages.iter()
        .filter(|msg| decoded_texts.contains(&msg.to_string()))
        .collect();

    let false_positives: Vec<_> = decoded_texts.iter()
        .filter(|msg| !all_expected.contains(&msg.as_str()))
        .collect();

    // Fail if any required messages are missing
    assert!(missing_required.is_empty(),
        "Missing {} required messages: {:?}\nDecoded: {:?}",
        missing_required.len(), missing_required, decoded_texts);

    // Fail if there are false positives
    assert!(false_positives.is_empty(),
        "Unexpected messages decoded (false positives): {:?}", false_positives);

    // Promote any todo_messages that now decode to required_messages
    assert!(todo_decoded.is_empty(),
        "TODO messages now decoding (move to required_messages): {:?}", todo_decoded);

    // Performance regression check
    let max_duration_secs = 90;
    assert!(decode_duration.as_secs() <= max_duration_secs,
        "Performance regression: decode took {}s, limit is {}s",
        decode_duration.as_secs(), max_duration_secs);
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
