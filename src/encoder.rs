//! FT8 Encoder
//!
//! High-level encoder that orchestrates the complete FT8 encoding pipeline:
//! Text → 77-bit message → CRC-14 → LDPC(174,91) → Symbol mapping → Audio synthesis
//!
//! # Example
//!
//! ```no_run
//! use rustyft8::encoder::encode_ft8;
//!
//! // Encode a message to symbols
//! let symbols = encode_ft8("CQ N0YPR DM42")?;
//! assert_eq!(symbols.len(), 79);
//! # Ok::<(), String>(())
//! ```

use bitvec::prelude::*;
use crate::crc::crc14;
use crate::ldpc;
use crate::message;
use crate::symbol;
use crate::sync::synthesize::synthesize_ft8_signal;

/// Encode FT8 message to symbol sequence
///
/// This is the main encoding function that takes a text message and produces
/// 79 FT8 symbols (tones 0-7) ready for audio synthesis.
///
/// Uses the global callsign hash cache automatically for non-standard callsigns.
///
/// # Pipeline
/// 1. Parse and encode text to 77-bit message
/// 2. Calculate and append 14-bit CRC → 91 bits
/// 3. LDPC encode to 174 bits
/// 4. Map to 79 symbols with Gray coding and Costas sync
///
/// # Arguments
/// * `text` - Message text (e.g., "CQ N0YPR DM42")
///
/// # Returns
/// * `Ok([u8; 79])` - 79 symbol array (tones 0-7)
/// * `Err(String)` - Error message if encoding fails
///
/// # Example
/// ```no_run
/// use rustyft8::encoder::encode_ft8;
///
/// let symbols = encode_ft8("CQ N0YPR DM42")?;
/// # Ok::<(), String>(())
/// ```
pub fn encode_ft8(text: &str) -> Result<[u8; 79], String> {
    // Step 1: Encode text to 77-bit message
    let mut msg77_storage = bitarr![u8, Msb0; 0; 80];
    let msg77 = &mut msg77_storage[..77];
    message::encode(text, msg77)?;

    // Step 2: Calculate CRC-14
    let crc = crc14(msg77);

    // Step 3: Combine message + CRC into 91-bit array
    let mut msg91_storage = bitarr![u8, Msb0; 0; 96];
    let msg91 = &mut msg91_storage[..91];
    msg91[..77].copy_from_bitslice(msg77);

    // Append 14-bit CRC
    for i in 0..14 {
        msg91.set(77 + i, ((crc >> (13 - i)) & 1) != 0);
    }

    // Step 4: LDPC encode to 174 bits
    let mut codeword_storage = bitarr![u8, Msb0; 0; 176];
    let codeword = &mut codeword_storage[..174];
    ldpc::encode(msg91, codeword);

    // Step 5: Map to 79 symbols
    let mut symbols = [0u8; 79];
    symbol::map(codeword, &mut symbols)?;

    Ok(symbols)
}

/// Encode FT8 message to audio samples
///
/// Performs complete encoding pipeline and generates normalized audio waveform.
/// The output is a complex baseband signal (I/Q samples) with unit amplitude.
///
/// For testing purposes, you can scale the amplitude or add noise after generation.
///
/// # Arguments
/// * `text` - Message text
/// * `frequency` - Center frequency in Hz (200-4000 Hz for FT8)
///
/// # Returns
/// * `Ok(Vec<(f32, f32)>)` - Complex audio samples (I/Q) with normalized amplitude
/// * `Err(String)` - Error message
///
/// # Example
/// ```no_run
/// use rustyft8::encoder::encode_ft8_audio;
///
/// let audio = encode_ft8_audio("CQ N0YPR DM42", 1500.0)?;
///
/// // For testing: scale amplitude or add noise
/// let scaled: Vec<_> = audio.iter()
///     .map(|(i, q)| (i * 0.5, q * 0.5))
///     .collect();
/// # Ok::<(), String>(())
/// ```
pub fn encode_ft8_audio(
    text: &str,
    frequency: f32,
) -> Result<Vec<(f32, f32)>, String> {
    // Encode to symbols
    let symbols = encode_ft8(text)?;

    // Synthesize audio at specified frequency
    const NMAX: usize = 15 * 12000; // Maximum signal length
    let mut output = vec![(0.0f32, 0.0f32); NMAX];

    let nsamples = synthesize_ft8_signal(&symbols, frequency, &mut output)?;

    // Truncate to actual length
    output.truncate(nsamples);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_ft8_basic() {
        let symbols = encode_ft8("CQ N0YPR DM42").unwrap();

        // Should produce 79 symbols
        assert_eq!(symbols.len(), 79);

        // All symbols should be 0-7
        for &sym in &symbols {
            assert!(sym <= 7, "Symbol {} out of range", sym);
        }

        // Check sync symbols (Costas arrays at positions 0-6, 36-42, 72-78)
        const COSTAS: [u8; 7] = [3, 1, 4, 0, 6, 5, 2];
        assert_eq!(&symbols[0..7], &COSTAS);
        assert_eq!(&symbols[36..43], &COSTAS);
        assert_eq!(&symbols[72..79], &COSTAS);
    }

    #[test]
    fn test_encode_ft8_known_message() {
        // Test against known encoding from WSJT-X
        // Message: "CQ SOTA N0YPR/R DM42"
        // Channel symbols from ft8code: 3140652 000671215006116571652175530543 140652 37542165575260315771541421243 3140652
        let expected_str = "3140652000671215006116571652175530543140652375421655752603157715414212433140652";
        let expected_symbols: Vec<u8> = expected_str.chars()
            .map(|c| c.to_digit(10).unwrap() as u8)
            .collect();

        let symbols = encode_ft8("CQ SOTA N0YPR/R DM42").unwrap();

        assert_eq!(symbols.len(), 79);
        assert_eq!(expected_symbols.len(), 79);
        assert_eq!(&symbols[..], &expected_symbols[..]);
    }

    #[test]
    fn test_encode_ft8_audio_generation() {
        let audio = encode_ft8_audio("CQ N0YPR DM42", 1500.0).unwrap();

        // Should produce audio samples
        assert!(!audio.is_empty());

        // FT8 transmission is 79 symbols × 1920 samples/symbol = 151,680 samples
        // (12000 Hz sample rate, 0.16 seconds per symbol)
        assert_eq!(audio.len(), 79 * 1920);

        // Check that audio was generated (has non-zero amplitude)
        let max_amplitude = audio.iter()
            .map(|(i, q)| (i * i + q * q).sqrt())
            .fold(0.0f32, f32::max);

        assert!(max_amplitude > 0.0, "Audio should have non-zero amplitude");
        assert!(max_amplitude <= 1.5, "Amplitude should be normalized (around 1.0)");
    }

    #[test]
    fn test_encode_different_frequencies() {
        let frequencies = [500.0, 1000.0, 1500.0, 2000.0, 2500.0];

        for _freq in frequencies {
            // Just test that encoding works - frequency doesn't affect symbol generation
            let result = encode_ft8("CQ N0YPR DM42");
            assert!(result.is_ok(), "Failed to encode");
        }
    }

    #[test]
    fn test_encode_invalid_message() {
        // Invalid callsign format should fail
        let result = encode_ft8("INVALID@@CALLSIGN@@@");
        assert!(result.is_err(), "Expected encoding to fail for invalid message");
    }

    #[test]
    fn test_encode_multiple_messages() {
        let messages = [
            "CQ N0YPR DM42",
            "N0YPR K1ABC RR73",
            "K1ABC N0YPR -15",
            "73",
        ];

        for msg in messages {
            let result = encode_ft8(msg);
            assert!(result.is_ok(), "Failed to encode: {}", msg);
        }
    }
}
