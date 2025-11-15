//! WAV File Generation
//!
//! This module provides functionality to write FT8 waveforms to WAV audio files.
//!
//! **WAV Format**:
//! - 16-bit PCM (signed integer samples)
//! - 12 kHz sample rate (standard for FT8)
//! - Mono channel
//!
//! **Usage**:
//! The module can operate in two modes:
//! - With `std`: Write directly to files using `write_wav_file`
//! - Without `std`: Generate WAV bytes using `generate_wav_bytes`

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;

/// WAV file header structure (44 bytes for 16-bit PCM mono)
struct WavHeader {
    sample_rate: u32,
    num_samples: u32,
}

impl WavHeader {
    fn new(sample_rate: u32, num_samples: u32) -> Self {
        Self {
            sample_rate,
            num_samples,
        }
    }

    /// Generate the 44-byte WAV header
    fn to_bytes(&self) -> [u8; 44] {
        let mut header = [0u8; 44];
        let data_size = self.num_samples * 2; // 2 bytes per sample (16-bit)
        let file_size = data_size + 36; // File size - 8 bytes

        // RIFF chunk descriptor
        header[0..4].copy_from_slice(b"RIFF");
        header[4..8].copy_from_slice(&file_size.to_le_bytes());
        header[8..12].copy_from_slice(b"WAVE");

        // fmt sub-chunk
        header[12..16].copy_from_slice(b"fmt ");
        header[16..20].copy_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
        header[20..22].copy_from_slice(&1u16.to_le_bytes()); // AudioFormat (1 = PCM)
        header[22..24].copy_from_slice(&1u16.to_le_bytes()); // NumChannels (1 = mono)
        header[24..28].copy_from_slice(&self.sample_rate.to_le_bytes());

        let byte_rate = self.sample_rate * 2; // SampleRate * NumChannels * BitsPerSample/8
        header[28..32].copy_from_slice(&byte_rate.to_le_bytes());
        header[32..34].copy_from_slice(&2u16.to_le_bytes()); // BlockAlign (NumChannels * BitsPerSample/8)
        header[34..36].copy_from_slice(&16u16.to_le_bytes()); // BitsPerSample

        // data sub-chunk
        header[36..40].copy_from_slice(b"data");
        header[40..44].copy_from_slice(&data_size.to_le_bytes());

        header
    }
}

/// Convert floating-point samples [-1.0, 1.0] to 16-bit PCM samples
///
/// Clamps values outside the range and scales to i16 range [-32768, 32767].
fn f32_to_i16(sample: f32) -> i16 {
    let clamped = if sample > 1.0 {
        1.0
    } else if sample < -1.0 {
        -1.0
    } else {
        sample
    };

    (clamped * 32767.0) as i16
}

/// Generate WAV file bytes from floating-point samples
///
/// Converts the waveform to 16-bit PCM format and wraps it with a WAV header.
///
/// # Arguments
/// * `samples` - Floating-point audio samples in range [-1.0, 1.0]
/// * `sample_rate` - Sample rate in Hz (typically 12000 for FT8)
///
/// # Returns
/// * `Vec<u8>` - Complete WAV file as bytes
///
/// # Example
/// ```
/// use rustyft8::wav;
///
/// let samples = vec![0.0f32; 151680]; // 79 symbols * 1920 samples/symbol
/// let wav_bytes = wav::generate_wav_bytes(&samples, 12000);
/// // Write wav_bytes to file or transmit over network
/// ```
pub fn generate_wav_bytes(samples: &[f32], sample_rate: u32) -> Vec<u8> {
    let num_samples = samples.len() as u32;
    let header = WavHeader::new(sample_rate, num_samples);
    let header_bytes = header.to_bytes();

    let mut wav_data = Vec::with_capacity(44 + (num_samples as usize * 2));
    wav_data.extend_from_slice(&header_bytes);

    for &sample in samples {
        let pcm_sample = f32_to_i16(sample);
        wav_data.extend_from_slice(&pcm_sample.to_le_bytes());
    }

    wav_data
}

/// Write WAV file to disk (requires std feature)
///
/// Generates a WAV file from floating-point samples and writes it to the specified path.
///
/// # Arguments
/// * `path` - File path for the output WAV file
/// * `samples` - Floating-point audio samples in range [-1.0, 1.0]
/// * `sample_rate` - Sample rate in Hz (typically 12000 for FT8)
///
/// # Returns
/// * `Result<(), String>` - Success or error message
///
/// # Example
/// ```no_run
/// use rustyft8::wav;
///
/// let samples = vec![0.0f32; 151680];
/// wav::write_wav_file("output.wav", &samples, 12000)?;
/// # Ok::<(), String>(())
/// ```
#[cfg(any(feature = "std", test))]
pub fn write_wav_file(path: &str, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    extern crate std;
    use std::fs::File;
    use std::io::Write;

    let wav_bytes = generate_wav_bytes(samples, sample_rate);

    let mut file = File::create(path)
        .map_err(|e| alloc::format!("Failed to create file '{}': {}", path, e))?;

    file.write_all(&wav_bytes)
        .map_err(|e| alloc::format!("Failed to write to file '{}': {}", path, e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_header_size() {
        let header = WavHeader::new(12000, 1000);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 44, "WAV header must be exactly 44 bytes");
    }

    #[test]
    fn test_wav_header_riff_chunk() {
        let header = WavHeader::new(12000, 1000);
        let bytes = header.to_bytes();

        // Check RIFF signature
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
    }

    #[test]
    fn test_wav_header_fmt_chunk() {
        let header = WavHeader::new(12000, 1000);
        let bytes = header.to_bytes();

        // Check fmt chunk
        assert_eq!(&bytes[12..16], b"fmt ");

        // Audio format (PCM = 1)
        let audio_format = u16::from_le_bytes([bytes[20], bytes[21]]);
        assert_eq!(audio_format, 1, "Audio format should be PCM (1)");

        // Number of channels (mono = 1)
        let num_channels = u16::from_le_bytes([bytes[22], bytes[23]]);
        assert_eq!(num_channels, 1, "Should be mono (1 channel)");

        // Sample rate
        let sample_rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
        assert_eq!(sample_rate, 12000, "Sample rate should be 12000 Hz");

        // Bits per sample
        let bits_per_sample = u16::from_le_bytes([bytes[34], bytes[35]]);
        assert_eq!(bits_per_sample, 16, "Should be 16-bit samples");
    }

    #[test]
    fn test_wav_header_data_chunk() {
        let header = WavHeader::new(12000, 1000);
        let bytes = header.to_bytes();

        // Check data chunk signature
        assert_eq!(&bytes[36..40], b"data");

        // Data size should be num_samples * 2 (16-bit = 2 bytes per sample)
        let data_size = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
        assert_eq!(data_size, 2000, "Data size should be 1000 samples * 2 bytes = 2000");
    }

    #[test]
    fn test_f32_to_i16_conversion() {
        // Test exact conversions
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(1.0), 32767);
        assert_eq!(f32_to_i16(-1.0), -32767);

        // Test clamping
        assert_eq!(f32_to_i16(1.5), 32767); // Clamps to 1.0
        assert_eq!(f32_to_i16(-1.5), -32767); // Clamps to -1.0

        // Test intermediate values
        let half = f32_to_i16(0.5);
        assert!(half > 16000 && half < 17000, "0.5 should be ~16383");
    }

    #[test]
    fn test_generate_wav_bytes_size() {
        let samples = vec![0.0f32; 1000];
        let wav_bytes = generate_wav_bytes(&samples, 12000);

        // Header (44 bytes) + data (1000 samples * 2 bytes/sample)
        assert_eq!(wav_bytes.len(), 44 + 2000);
    }

    #[test]
    fn test_generate_wav_bytes_header() {
        let samples = vec![0.0f32; 100];
        let wav_bytes = generate_wav_bytes(&samples, 12000);

        // Check RIFF header
        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");
        assert_eq!(&wav_bytes[12..16], b"fmt ");
        assert_eq!(&wav_bytes[36..40], b"data");
    }

    #[test]
    fn test_generate_wav_bytes_silence() {
        let samples = vec![0.0f32; 10];
        let wav_bytes = generate_wav_bytes(&samples, 12000);

        // Check that all audio samples are zero
        for i in 0..10 {
            let offset = 44 + (i * 2);
            let sample = i16::from_le_bytes([wav_bytes[offset], wav_bytes[offset + 1]]);
            assert_eq!(sample, 0, "Silent samples should be zero");
        }
    }

    #[test]
    fn test_generate_wav_bytes_full_scale() {
        let samples = vec![1.0f32, -1.0f32, 0.5f32, -0.5f32];
        let wav_bytes = generate_wav_bytes(&samples, 12000);

        // Check first sample (1.0 -> 32767)
        let sample0 = i16::from_le_bytes([wav_bytes[44], wav_bytes[45]]);
        assert_eq!(sample0, 32767);

        // Check second sample (-1.0 -> -32767)
        let sample1 = i16::from_le_bytes([wav_bytes[46], wav_bytes[47]]);
        assert_eq!(sample1, -32767);

        // Check third sample (0.5 -> ~16383)
        let sample2 = i16::from_le_bytes([wav_bytes[48], wav_bytes[49]]);
        assert!(sample2 > 16000 && sample2 < 17000);

        // Check fourth sample (-0.5 -> ~-16383)
        let sample3 = i16::from_le_bytes([wav_bytes[50], wav_bytes[51]]);
        assert!(sample3 < -16000 && sample3 > -17000);
    }

    #[test]
    fn test_generate_wav_bytes_ft8_size() {
        // FT8 transmission: 79 symbols * 1920 samples/symbol = 151,680 samples
        let samples = vec![0.0f32; 79 * 1920];
        let wav_bytes = generate_wav_bytes(&samples, 12000);

        // Header (44) + data (151680 * 2)
        assert_eq!(wav_bytes.len(), 44 + (151680 * 2));
    }

    #[cfg(any(feature = "std", test))]
    #[test]
    fn test_write_wav_file() {
        extern crate std;
        use std::fs;

        let samples = vec![0.5f32; 1000];
        let temp_path = "/tmp/test_rustyft8.wav";

        // Write file
        let result = write_wav_file(temp_path, &samples, 12000);
        assert!(result.is_ok(), "Failed to write WAV file: {:?}", result);

        // Check file exists and has correct size
        let metadata = fs::metadata(temp_path).expect("File should exist");
        assert_eq!(metadata.len(), 44 + 2000);

        // Clean up
        fs::remove_file(temp_path).ok();
    }
}
