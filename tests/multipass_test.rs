use rustyft8::{decode_ft8, decode_ft8_multipass, DecoderConfig};
use std::collections::HashSet;

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
fn test_multipass_on_real_recording() {
    // Load the real FT8 recording (WSJT-X decodes 22 messages from this)
    let wav_data = read_wav_file("tests/test_data/210703_133430.wav")
        .expect("Failed to load test recording");

    let config = DecoderConfig::default();

    println!("\n=== Single-Pass Decoding ===");
    let mut single_pass_messages = Vec::new();
    decode_ft8(&wav_data, &config, |msg| {
        println!("{:4.0} Hz  {:+3} dB  {}", msg.frequency, msg.snr_db, msg.message);
        single_pass_messages.push(msg.message.clone());
        true
    }).expect("Single-pass decode failed");

    let single_pass_count = single_pass_messages.len();
    println!("\nSingle-pass: {} decodes", single_pass_count);

    println!("\n=== Multi-Pass Decoding (3 passes) ===");
    let mut multipass_messages = Vec::new();
    decode_ft8_multipass(&wav_data, &config, 3, |msg| {
        println!("{:4.0} Hz  {:+3} dB  {}", msg.frequency, msg.snr_db, msg.message);
        multipass_messages.push(msg.message.clone());
        true
    }).expect("Multi-pass decode failed");

    let multipass_count = multipass_messages.len();
    println!("\nMulti-pass: {} decodes", multipass_count);

    // Check that multi-pass found at least as many as single-pass
    assert!(
        multipass_count >= single_pass_count,
        "Multi-pass should find at least as many decodes as single-pass"
    );

    // Find messages that were only decoded in multi-pass
    let single_set: HashSet<_> = single_pass_messages.iter().collect();
    let multipass_set: HashSet<_> = multipass_messages.iter().collect();

    let new_decodes: Vec<_> = multipass_set.difference(&single_set).collect();

    if !new_decodes.is_empty() {
        println!("\n=== New decodes from multi-pass ===");
        for msg in &new_decodes {
            println!("  {}", msg);
        }
        println!("Multi-pass revealed {} additional signals", new_decodes.len());
    }

    println!("\n=== Summary ===");
    println!("Single-pass:  {} decodes", single_pass_count);
    println!("Multi-pass:   {} decodes (+{})", multipass_count, multipass_count - single_pass_count);
    println!("WSJT-X:       22 decodes (target)");
    println!("Gap remaining: {} decodes", 22i32 - multipass_count as i32);
}
