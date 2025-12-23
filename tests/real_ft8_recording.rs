//! Integration tests using real FT8 recordings
//!
//! Tests the decoder against actual FT8 recordings to validate real-world performance.
//! Reference recordings are compared against WSJT-X output for validation.
//!
//! Test data is driven by tests/test_data/recordings.json

use rustyft8::{decode_ft8, DecoderConfig, DecodedMessage};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::{read_wav_file, normalize_signal_length};

#[derive(Debug, Serialize, Deserialize)]
struct ExpectedMessage {
    text: String,
    required: bool,
    #[serde(default)]
    wsjtx_hard_errors: Option<usize>,
    #[serde(default)]
    wsjtx_frequency: Option<f32>,
    #[serde(default)]
    wsjtx_dt: Option<f32>,
    #[serde(default)]
    expected_codeword: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordingTest {
    name: String,
    description: String,
    wav_file: String,
    sample_rate: u32,
    duration_seconds: u32,
    max_decode_time_seconds: u64,
    messages: Vec<ExpectedMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecordingsConfig {
    recordings: Vec<RecordingTest>,
}

fn load_recordings_config() -> RecordingsConfig {
    let json_path = "tests/test_data/recordings.json";
    let json_content = std::fs::read_to_string(json_path)
        .expect("Failed to read recordings.json");
    serde_json::from_str(&json_content)
        .expect("Failed to parse recordings.json")
}

fn test_recording(recording: &RecordingTest) {
    println!("\n=== Testing: {} ===", recording.name);
    println!("{}", recording.description);

    let signal = read_wav_file(&recording.wav_file)
        .expect(&format!("Failed to read WAV file: {}", recording.wav_file));
    let signal_15s = normalize_signal_length(signal);
    let config = DecoderConfig::default();

    let mut decoded_messages: Vec<DecodedMessage> = Vec::new();
    let start_time = Instant::now();
    let _count = decode_ft8(&signal_15s, &config, |msg| {
        decoded_messages.push(msg);
        true
    }).expect("Decode failed");
    let decode_duration = start_time.elapsed();

    // Separate required and optional messages
    let required_messages: Vec<&str> = recording.messages.iter()
        .filter(|m| m.required)
        .map(|m| m.text.as_str())
        .collect();

    let optional_messages: Vec<&str> = recording.messages.iter()
        .filter(|m| !m.required)
        .map(|m| m.text.as_str())
        .collect();

    let all_expected: Vec<&str> = recording.messages.iter()
        .map(|m| m.text.as_str())
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

    let optional_decoded: Vec<_> = optional_messages.iter()
        .filter(|msg| decoded_texts.contains(&msg.to_string()))
        .collect();

    let false_positives: Vec<_> = decoded_texts.iter()
        .filter(|msg| !all_expected.contains(&msg.as_str()))
        .collect();

    // Report results
    println!("\nResults:");
    println!("  Decoded: {} messages", decoded_texts.len());
    println!("  Required: {}/{}", required_messages.len() - missing_required.len(), required_messages.len());
    println!("  Optional: {}/{}", optional_decoded.len(), optional_messages.len());
    println!("  Decode time: {:.1}s", decode_duration.as_secs_f64());

    // Build map of expected messages with their WSJT-X hard errors
    let wsjtx_errors: std::collections::HashMap<&str, usize> = recording.messages.iter()
        .filter_map(|m| m.wsjtx_hard_errors.map(|e| (m.text.as_str(), e)))
        .collect();

    // Compare hard errors for decoded messages
    println!("\nHard Error Comparison (WSJT-X vs RustyFt8):");
    let mut total_wsjtx = 0usize;
    let mut total_ours = 0usize;
    let mut count = 0usize;

    for msg in &decoded_messages {
        if let Some(&wsjtx_errs) = wsjtx_errors.get(msg.message.as_str()) {
            let ours = msg.hard_errors;
            let diff: i32 = ours as i32 - wsjtx_errs as i32;
            let indicator = if diff <= 0 { "✓" } else if diff <= 5 { "~" } else { "✗" };
            println!("  {} {:30} WSJT-X:{:2} Ours:{:2} (diff: {:+3})",
                indicator, msg.message, wsjtx_errs, ours, diff);
            total_wsjtx += wsjtx_errs;
            total_ours += ours;
            count += 1;
        }
    }

    if count > 0 {
        println!("\n  Summary: WSJT-X avg={:.1}, Ours avg={:.1}, Gap={:.1}",
            total_wsjtx as f64 / count as f64,
            total_ours as f64 / count as f64,
            (total_ours as f64 - total_wsjtx as f64) / count as f64);
    }

    // Fail if any required messages are missing
    assert!(missing_required.is_empty(),
        "Missing {} required messages: {:?}\nDecoded: {:?}",
        missing_required.len(), missing_required, decoded_texts);

    // Fail if there are false positives
    assert!(false_positives.is_empty(),
        "Unexpected messages decoded (false positives): {:?}", false_positives);

    // Report optional messages that now decode (good news!)
    if !optional_decoded.is_empty() {
        println!("\n✓ Progress: Optional messages now decoding (consider marking as required):");
        for msg in &optional_decoded {
            println!("    {}", msg);
        }
        panic!("Optional messages now decoding (move to required in recordings.json): {:?}", optional_decoded);
    }

    // Performance regression check
    if decode_duration.as_secs() > recording.max_decode_time_seconds {
        panic!("Performance regression: decode took {}s, limit is {}s",
            decode_duration.as_secs(), recording.max_decode_time_seconds);
    }
}

#[test]
#[ignore] // Slow test - run with: cargo test --release --test real_ft8_recording -- --ignored
fn test_all_real_ft8_recordings() {
    let config = load_recordings_config();

    println!("Loaded {} recording test(s)", config.recordings.len());

    for recording in &config.recordings {
        test_recording(recording);
    }

    println!("\n✓ All {} recording test(s) passed", config.recordings.len());
}

