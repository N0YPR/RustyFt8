// Core modules
mod callsign;
mod grid;
mod callsign_cache;

// Refactored message77 modules
mod constants;
mod types;
mod lookup_tables;
mod text_encoding;
mod parser;
mod encode;
mod decode;

// Public API - minimal surface area
// Only expose the high-level encode/decode functions and the hash cache
pub use callsign_cache::CallsignHashCache;
pub use callsign::is_valid_callsign;

// Internal imports - not exported
use encode::encode_variant;
use decode::decode_message_bits;
use parser::parse_message_variant;

use bitvec::prelude::*;

/// Encode a text message into a 77-bit FT8 message
///
/// This parses the text and encodes it into 77 bits.
/// The encoder determines the appropriate message type (i3.n3) based on what fits.
///
/// # Arguments
/// * `text` - The message text (e.g., "CQ N0YPR DM42")
/// * `output` - Mutable bit slice to write the 77 bits into (must be exactly 77 bits)
/// * `cache` - Mutable reference to a CallsignHashCache for non-standard callsigns
///
/// # Examples
///
/// ```no_run
/// use bitvec::prelude::*;
/// use rustyft8::message::{encode, CallsignHashCache};
///
/// let mut cache = CallsignHashCache::new();
/// let mut storage = bitarr![u8, Msb0; 0; 80];  // 10 bytes
/// encode("CQ N0YPR DM42", &mut storage[0..77], &mut cache)?;
///
/// // Non-standard callsigns are automatically cached
/// encode("K1ABC RR73; W9XYZ <KH1/KH7Z> -08", &mut storage[0..77], &mut cache)?;
/// # Ok::<(), String>(())
/// ```
pub fn encode(text: &str, output: &mut BitSlice<u8, Msb0>, cache: &mut CallsignHashCache) -> Result<(), String> {
    if output.len() != 77 {
        return Err(format!("Output buffer must be exactly 77 bits, got {}", output.len()));
    }

    // 1. Parse text into MessageVariant (internal detail)
    let variant = parse_message_variant(text)?;

    // 2. Encode variant into 77 bits
    encode_variant(&variant, output, Some(cache))?;

    Ok(())
}

/// Decode a 77-bit FT8 message back to text
///
/// This reverses the encoding process, extracting the message type and fields
/// from the bit array and reconstructing the original text.
///
/// Note: The decoded text may differ from the original input due to encoding
/// limitations. For example:
/// - "CQ PJ4/K1ABC FN42" → decodes as "CQ K1ABC FN42" (prefix stripped)
/// - "CQ PJ4/K1ABC" → decodes as "CQ PJ4/K1ABC" (uses Type 4 encoding)
///
/// # Arguments
/// * `bits` - The 77-bit message (must be exactly 77 bits)
/// * `cache` - Optional reference to a CallsignHashCache for resolving DXpedition mode hashes
///
/// # Examples
///
/// ```no_run
/// use bitvec::prelude::*;
/// use rustyft8::message::{encode, decode, CallsignHashCache};
///
/// let mut cache = CallsignHashCache::new();
/// let mut storage = bitarr![u8, Msb0; 0; 80];
/// encode("CQ N0YPR DM42", &mut storage[0..77], &mut cache)?;
/// let text = decode(&storage[0..77], None)?;
/// assert_eq!(text, "CQ N0YPR DM42");
/// # Ok::<(), String>(())
/// ```
pub fn decode(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    if bits.len() != 77 {
        return Err(format!("Input must be exactly 77 bits, got {}", bits.len()));
    }

    decode_message_bits(bits, cache)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests use std
    #[cfg(test)]
    use std::vec::Vec;
    #[cfg(test)]
    use std::string::String;

    /// Test case structure for message encoding and decoding
    #[derive(Debug, Clone)]
    struct MessageTestCase {
        /// Input message text
        message: String,
        /// Expected 77-bit output from ft8code (as binary string)
        /// Note: This is the source-encoded message, NOT including CRC or parity
        expected_bits: String,
        /// What the message decodes to (may differ from input for nonstandard callsigns)
        expected_decoded: String,
    }

    /// Parse test cases from CSV data
    /// Format: message,expected_bits,expected_decoded
    /// Lines starting with # are comments and empty lines are ignored
    fn parse_test_cases(csv_data: &str) -> Vec<MessageTestCase> {
        csv_data
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
            .map(|line| {
                let parts: Vec<&str> = line.splitn(3, ',').collect();
                assert_eq!(parts.len(), 3, "Invalid CSV line: {}", line);
                MessageTestCase {
                    message: parts[0].to_string(),
                    expected_bits: parts[1].to_string(),
                    expected_decoded: parts[2].to_string(),
                }
            })
            .collect()
    }

    /// Load test cases from embedded CSV file
    fn test_cases() -> Vec<MessageTestCase> {
        const TEST_DATA: &str = include_str!("../../tests/message/encode_decode_cases.csv");
        parse_test_cases(TEST_DATA)
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let cases = test_cases();
        let mut cache = CallsignHashCache::new();

        for (idx, test) in cases.iter().enumerate() {
            // Print test case for better debugging
            eprintln!("\n[Test Case {}] Testing: \"{}\"", idx, test.message);

            // Create storage for the 77-bit message
            let mut storage = bitarr![u8, Msb0; 0; 80];  // 10 bytes

            // Encode the message
            encode(&test.message, &mut storage[0..77], &mut cache)
                .unwrap_or_else(|e| panic!(
                    "\n❌ ENCODE FAILED [Case {}]\n   Message: \"{}\"\n   Error: {}\n",
                    idx, test.message, e
                ));

            // Convert our bits to binary string for comparison
            let our_bits = bits_to_string(&storage[0..77]);

            // Compare against reference
            assert_eq!(
                our_bits, test.expected_bits,
                "\n❌ BIT ENCODING MISMATCH [Case {}] \"{}\"\n   Expected: {}\n   Actual:   {}",
                idx, test.message, test.expected_bits, our_bits
            );

            // Decode and verify the decoded text matches expected
            let decoded_text = decode(&storage[0..77], Some(&cache))
                .unwrap_or_else(|e| panic!(
                    "\n❌ DECODE FAILED [Case {}]\n   Message: \"{}\"\n   Error: {}\n",
                    idx, test.message, e
                ));

            assert_eq!(
                decoded_text, test.expected_decoded,
                "\n❌ DECODED TEXT MISMATCH [Case {}]\n   Original message: \"{}\"\n   Expected decoded: \"{}\"\n   Actual decoded:   \"{}\"",
                idx, test.message, test.expected_decoded, decoded_text
            );

            eprintln!("   ✓ Passed");
        }

        eprintln!("\n✅ All {} test cases passed!", cases.len());
    }

    /// Convert a BitSlice to a binary string (LSB-first: bit 0 on left)
    fn bits_to_string(bits: &BitSlice<u8, Msb0>) -> String {
        let mut s = String::with_capacity(bits.len());
        for i in 0..bits.len() {
            s.push(if bits[i] { '1' } else { '0' });
        }
        s
    }

    #[test]
    fn test_bits_to_string_conversion() {
        let mut bits = BitArray::<[u8; 1], Msb0>::ZERO;
        bits.set(0, true);
        bits.set(2, true);
        bits.set(4, true);

        let s = bits_to_string(&bits[0..5]);
        assert_eq!(s, "10101");
    }
}
