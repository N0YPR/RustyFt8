//! Integration tests comparing RustyFt8 encoding against WSJT-X ft8code output
//!
//! These tests validate the complete message encoding pipeline by comparing
//! our 77-bit output against reference values from ft8code.
//!
//! ## Data-Driven Testing
//!
//! This module contains tests for the FT8 message encoding and decoding functionality.
//! 
//! Test cases are stored in `tests/message/encode_decode_cases.csv` for easy maintenance.
//! See `tests/test_data/README.md` for details on adding test cases.
//!
//! The test automatically picks up all cases from the CSV file - just add rows and run!
//!
//! Quick start:
//! ```bash
//! # Add a test case using the helper script
//! ./scripts/add_test_case.sh "CQ DX K1ABC FN42"
//! ```
//!
//! To generate test cases, use the helper script:
//! ```bash
//! ./scripts/add_test_case.sh "MESSAGE TEXT"
//! ```

// Tests use std
#[cfg(test)]
use std::vec::Vec;
#[cfg(test)]
use std::string::String;

use super::{encode, decode};
use bitvec::prelude::*;

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

/// Generate rstest cases from embedded CSV file
fn test_cases() -> Vec<MessageTestCase> {
    const TEST_DATA: &str = include_str!("../../tests/message/encode_decode_cases.csv");
    parse_test_cases(TEST_DATA)
}

#[test]
fn test_encode_decode_roundtrip() {
    let cases = test_cases();
    let mut cache = crate::message::CallsignHashCache::new();
    
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
