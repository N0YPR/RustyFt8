use rustyft8::{decode_ft8, DecoderConfig};
use hound;

#[test]
fn test_real_ft8_recording() {
    // Read the real FT8 recording
    let wav_path = "tests/test_data/210703_133430.wav";
    let mut reader = hound::WavReader::open(wav_path)
        .expect("Failed to open WAV file");

    let spec = reader.spec();
    println!("WAV format: {} Hz, {} channels, {} bits",
             spec.sample_rate, spec.channels, spec.bits_per_sample);

    // Read samples and convert to f32
    let samples: Vec<f32> = reader.samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();

    println!("Read {} samples ({:.2} seconds)",
             samples.len(), samples.len() as f32 / spec.sample_rate as f32);

    // Decode the recording
    let config = DecoderConfig::default();

    let mut decoded_messages = Vec::new();
    let count = decode_ft8(&samples, &config, |msg| {
        println!("Decoded: {}", msg.message);
        decoded_messages.push(msg.message.clone());
        true
    }).expect("Decode failed");

    println!("\nTotal signals decoded: {}", count);

    // Verify we decoded at least some signals
    assert!(count > 0, "Should decode at least one signal from real recording");
    assert!(!decoded_messages.is_empty(), "Should have decoded messages");
}
