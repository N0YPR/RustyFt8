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
//! ## Wildcard support
//!
//! Use any non-'0'/'1' character as a wildcard in `expected_bits` to skip checking those bits.
//! This lets you test only the fields you've implemented so far.
//!
//! Examples:
//! ```
//! expected_bits: "0000000000000000000000000010............................xxxxxxxxxxxxxx001",  // CQ + i3
//! expected_bits: "0000000000000000000000000010............................011111100100011001",  // CQ + grid + i3
//! expected_bits: "00000000000000000000000000100000010100100110110011100110100001100111110010001",  // All fields
//! ```
//!
//! To generate test cases, use ft8code:
//! ```bash
//! /workspaces/RustyFt8/wsjtx/wsjtx-2.7.0/build/wsjtx-prefix/src/wsjtx-build/ft8code "MESSAGE TEXT"
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
        encode(&test.message, &mut storage[0..77], Some(&mut cache))
            .unwrap_or_else(|e| panic!(
                "\n❌ ENCODE FAILED [Case {}]\n   Message: \"{}\"\n   Error: {}\n", 
                idx, test.message, e
            ));
        
        // Convert our bits to binary string for comparison
        let our_bits = bits_to_string(&storage[0..77]);
        
        // Compare against reference (supporting wildcards)
        assert_bits_match_with_wildcards(
            &our_bits,
            &test.expected_bits,
            &format!("[Case {}] \"{}\"", idx, test.message)
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

/// Compare bit strings with wildcard support
/// 
/// Any character that is not '0' or '1' in the expected string is treated as a wildcard (don't care).
/// Common choices: '-', '_', ' ', 'x', '*', '.'
/// 
/// # Examples
/// ```
/// // Test only the last 3 bits (i3 field) - use whatever character you find readable
/// assert_bits_match_with_wildcards(
///     "00000000000000000000000000100000010100100110110011100110100001100111110010001",
///     "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx001",  // x for wildcards
///     "i3 field"
/// );
/// 
/// assert_bits_match_with_wildcards(
///     "00000000000000000000000000100000010100100110110011100110100001100111110010001",
///     "--------------------------------------------------------------------------001",  // or dashes
///     "i3 field"
/// );
/// 
/// assert_bits_match_with_wildcards(
///     "00000000000000000000000000100000010100100110110011100110100001100111110010001",
///     "..........................................................................001",  // or dots
///     "i3 field"
/// );
/// ```
fn assert_bits_match_with_wildcards(actual: &str, expected: &str, context: &str) {
    if actual.len() != expected.len() {
        panic!(
            "\n❌ BIT LENGTH MISMATCH {}\n   Expected: {} bits\n   Actual:   {} bits\n",
            context, expected.len(), actual.len()
        );
    }
    
    let mut mismatches = Vec::new();
    
    for (i, (actual_ch, expected_ch)) in actual.chars().zip(expected.chars()).enumerate() {
        // Any non-0/1 character is a wildcard
        if expected_ch != '0' && expected_ch != '1' {
            continue;
        }
        
        if actual_ch != expected_ch {
            mismatches.push((i, actual_ch, expected_ch));
        }
    }
    
    if !mismatches.is_empty() {
        let mut error_msg = format!("\n❌ BIT ENCODING MISMATCH {}\n", context);
        error_msg.push_str(&format!("\nBit mismatches: {} bit(s) don't match:\n", mismatches.len()));
        
        // Show first 10 mismatches to avoid overwhelming output
        for (bit_pos, actual_bit, expected_bit) in mismatches.iter().take(10) {
            error_msg.push_str(&format!(
                "   Bit {:2}: expected '{}', got '{}'\n",
                bit_pos, expected_bit, actual_bit
            ));
        }
        
        if mismatches.len() > 10 {
            error_msg.push_str(&format!("   ... and {} more\n", mismatches.len() - 10));
        }
        
        error_msg.push_str(&format!("\nExpected: {}\n", expected));
        error_msg.push_str(&format!("Actual:   {}\n", actual));
        
        // Show visual diff
        let mut diff = String::with_capacity(expected.len());
        for (a, e) in actual.chars().zip(expected.chars()) {
            if e != '0' && e != '1' {
                diff.push('.');  // Wildcard position
            } else if a == e {
                diff.push(' ');  // Match
            } else {
                diff.push('^');  // Mismatch
            }
        }
        error_msg.push_str(&format!("Diff:     {}\n", diff));
        error_msg.push_str("          (^ = mismatch, . = wildcard)\n");
        
        panic!("{}", error_msg);
    }
}

/// Assert that a specific bit range matches the expected pattern
/// 
/// This is useful for incremental implementation - test only the bits
/// you've implemented so far.
/// 
/// # Example
/// ```
/// // Test only the i3 field (bits 74-76)
/// assert_bits_match(msg.message_bits(), 74..77, "001", "i3 field");
/// 
/// // Test grid encoding (bits 59-73)
/// assert_bits_match(msg.message_bits(), 59..74, "011111100100011", "grid field");
/// ```
fn assert_bits_match(
    actual: &BitSlice<u8, Msb0>,
    range: core::ops::Range<usize>,
    expected: &str,
    field_name: &str,
) {
    let actual_str = bits_to_string(&actual[range.clone()]);
    assert_eq!(
        actual_str, expected,
        "\n{} mismatch (bits {}..{}):\nExpected: {}\nGot:      {}\n",
        field_name, range.start, range.end, expected, actual_str
    );
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

#[test]
fn test_wildcard_matching() {
    // All wildcards should pass - any non-0/1 character works
    assert_bits_match_with_wildcards("10101", "_____", "underscores");
    assert_bits_match_with_wildcards("10101", "-----", "dashes");
    assert_bits_match_with_wildcards("10101", "     ", "spaces");
    assert_bits_match_with_wildcards("10101", "xxxxx", "x's");
    assert_bits_match_with_wildcards("10101", "*****", "asterisks");
    assert_bits_match_with_wildcards("10101", ".....", "dots");
    
    // Mixed wildcards and exact matches
    assert_bits_match_with_wildcards("10101", "1xxx1", "first and last");
    assert_bits_match_with_wildcards("10101", "x0x0x", "even positions");
    assert_bits_match_with_wildcards("10101", "1-.-1", "mixed wildcards");
    assert_bits_match_with_wildcards("10101", "10101", "exact match");
}

#[test]
#[should_panic(expected = "Bit mismatches")]
fn test_wildcard_matching_failure() {
    assert_bits_match_with_wildcards("10101", "1___0", "should fail");
}

#[test]
fn test_assert_bits_match() {
    let mut bits = BitArray::<[u8; 2], Msb0>::ZERO;
    // Set bits to pattern: 10101
    bits.set(0, true);
    bits.set(2, true);
    bits.set(4, true);
    
    // This should pass
    assert_bits_match(&bits[..], 0..5, "10101", "test pattern");
    
    // Test a subset
    assert_bits_match(&bits[..], 2..5, "101", "subset pattern");
}

#[test]
#[should_panic(expected = "mismatch")]
fn test_assert_bits_match_failure() {
    let bits = BitArray::<[u8; 1], Msb0>::ZERO;
    assert_bits_match(&bits[..], 0..3, "111", "wrong pattern");
}
