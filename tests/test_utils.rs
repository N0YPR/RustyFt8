//! Shared utilities for integration tests

use hound;

/// Read a WAV file and convert to f32 samples normalized to [-1.0, 1.0]
pub fn read_wav_file(path: &str) -> Result<Vec<f32>, String> {
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

/// Pad or truncate signal to exactly 15 seconds (180,000 samples at 12 kHz)
pub fn normalize_signal_length(mut signal: Vec<f32>) -> Vec<f32> {
    let expected_samples = 15 * 12000;
    if signal.len() < expected_samples {
        signal.resize(expected_samples, 0.0);
    } else if signal.len() > expected_samples {
        signal.truncate(expected_samples);
    }
    signal
}
