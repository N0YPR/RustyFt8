//! Integration tests using real FT8 recordings
//!
//! Tests the decoder against actual FT8 recordings to validate real-world performance

use rustyft8::{decode_ft8, DecoderConfig, DecodedMessage};
use hound;

/// Read a WAV file and convert to f32 samples normalized to [-1.0, 1.0]
fn read_wav_file(path: &str) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV file: {}", e))?;

    let spec = reader.spec();

    // Verify format
    if spec.sample_rate != 12000 {
        return Err(format!("Expected 12000 Hz sample rate, got {}", spec.sample_rate));
    }

    if spec.channels != 1 {
        return Err(format!("Expected mono audio, got {} channels", spec.channels));
    }

    // Read and convert samples to f32
    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Int => {
            match spec.bits_per_sample {
                16 => {
                    // Read as i16 and normalize to [-1.0, 1.0]
                    reader.into_samples::<i16>()
                        .map(|s| s.map(|v| v as f32 / 32768.0))
                        .collect()
                }
                _ => return Err(format!("Unsupported bit depth: {}", spec.bits_per_sample)),
            }
        }
        hound::SampleFormat::Float => {
            reader.into_samples::<f32>().collect()
        }
    };

    samples.map_err(|e| format!("Failed to read samples: {}", e))
}

#[test]
fn test_real_ft8_recording_210703_133430() {
    // Read the real FT8 recording
    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV file");

    println!("Read {} samples from {}", signal.len(), wav_path);

    // Ensure we have at least 15 seconds at 12 kHz (180,000 samples)
    let expected_min_samples = 15 * 12000;
    assert!(signal.len() >= expected_min_samples,
        "Recording too short: {} samples (need at least {})",
        signal.len(), expected_min_samples);

    // Pad or truncate to exactly 15 seconds if needed
    let mut signal_15s = signal;
    if signal_15s.len() < expected_min_samples {
        signal_15s.resize(expected_min_samples, 0.0);
    } else if signal_15s.len() > expected_min_samples {
        signal_15s.truncate(expected_min_samples);
    }

    // Decode with standard configuration
    let config = DecoderConfig::default();

    let mut decoded_messages: Vec<DecodedMessage> = Vec::new();
    let count = decode_ft8(&signal_15s, &config, |msg| {
        println!("Decoded: {} @ {:.1} Hz, DT={:.2}s, SNR={} dB, sync={:.2}, LDPC iters={}, LLR scale={:.1}, nsym={}",
            msg.message, msg.frequency, msg.time_offset, msg.snr_db,
            msg.sync_power, msg.ldpc_iterations, msg.llr_scale, msg.nsym);
        decoded_messages.push(msg);
        true // Continue decoding
    }).expect("Decode failed");

    println!("\nTotal decoded: {} messages", count);

    // We should decode at least some messages from a real recording
    assert!(count > 0, "No messages decoded from real recording");

    // Verify all decoded messages are non-empty
    for msg in &decoded_messages {
        assert!(!msg.message.is_empty(), "Decoded empty message");
        assert!(msg.frequency > 0.0, "Invalid frequency");
    }

    println!("\n✓ Successfully decoded {} messages from real FT8 recording", count);
}

#[test]
fn test_real_recording_with_custom_config() {
    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV file");

    // Pad to 15 seconds
    let mut signal_15s = signal;
    signal_15s.resize(15 * 12000, 0.0);

    // Test with a more aggressive configuration
    let config = DecoderConfig {
        freq_min: 100.0,
        freq_max: 3000.0,
        sync_threshold: 0.4,  // Lower threshold to catch weaker signals
        max_candidates: 200,
        decode_top_n: 100,
    };

    let mut count = 0;
    decode_ft8(&signal_15s, &config, |msg| {
        println!("Decoded (aggressive): {} @ {:.1} Hz, SNR={} dB",
            msg.message, msg.frequency, msg.snr_db);
        count += 1;
        true
    }).expect("Decode failed");

    println!("\nAggressive config decoded: {} messages", count);

    // With lower threshold, we should still decode something
    assert!(count > 0, "No messages decoded even with aggressive config");
}

#[test]
fn test_wav_reader_format_validation() {
    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV file");

    // Basic sanity checks on the signal
    assert_eq!(signal.len(), 15 * 12000, "Signal should be exactly 15 seconds");

    // Check signal is not all zeros (actual recording has content)
    let sum: f32 = signal.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "Signal appears to be empty");

    // Check reasonable amplitude range (normalized to [-1.0, 1.0])
    let max_amp = signal.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    assert!(max_amp <= 1.0, "Signal amplitude exceeds normalized range");

    println!("✓ WAV file format validated: {} samples, max amplitude: {:.3}",
        signal.len(), max_amp);
}
