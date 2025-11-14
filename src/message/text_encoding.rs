use alloc::string::{String, ToString};
use alloc::format;
use crate::message::constants::{CHARSET_BASE42, CHARSET_BASE38};

/// Encode compound callsign for Type 4 NonStandardCall (up to 11 characters) into 58 bits
/// Uses base-38 encoding with character set: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/'
///
/// This is used for encoding compound callsigns like "PJ4/K1ABC" or "KH1/KH7Z".
/// The callsign is right-padded to 11 characters before encoding.
pub fn encode_callsign_base38(callsign: &str) -> Result<u64, String> {
    const BASE: u64 = 38;

    if callsign.len() > 11 {
        return Err(format!("Callsign must be 11 characters or less, got {}", callsign.len()));
    }

    // Right-align the callsign with spaces (as per WSJT-X packjt77.f90)
    let padded = format!("{:>11}", callsign.to_uppercase());

    // Encode using base-38: accumulator = accumulator * 38 + char_index
    let mut acc: u64 = 0;

    for ch in padded.bytes() {
        // Find character index in charset
        let idx = CHARSET_BASE38.iter().position(|&c| c == ch)
            .ok_or_else(|| format!("Invalid character '{}' in callsign (valid: {})",
                                   ch as char,
                                   core::str::from_utf8(CHARSET_BASE38).unwrap()))?;

        // Multiply accumulator by 38 and add index
        acc = acc * BASE + idx as u64;
    }

    Ok(acc)
}

/// Decode 58 bits back to compound callsign (11 characters)
/// Uses base-38 decoding for Type 4 NonStandardCall messages
pub fn decode_callsign_base38(value: u64) -> Result<String, String> {
    const BASE: u64 = 38;

    let mut acc = value;
    let mut result = String::with_capacity(11);

    // Decode in reverse: extract character by dividing by 38
    for _ in 0..11 {
        let remainder = (acc % BASE) as usize;
        if remainder >= CHARSET_BASE38.len() {
            return Err(format!("Invalid base-38 value: remainder {} out of range", remainder));
        }
        result.push(CHARSET_BASE38[remainder] as char);
        acc /= BASE;
    }

    // Reverse since we decoded backwards, then trim spaces
    Ok(result.chars().rev().collect::<String>().trim().to_string())
}

/// Encode free text message (up to 13 characters) into 71 bits
/// Uses base-42 encoding with character set: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?'
///
/// This is used for Type 0 free text messages like "TNX BOB 73 GL".
/// The text is right-padded to 13 characters before encoding.
pub fn encode_free_text(text: &str) -> Result<[u8; 9], String> {
    const BASE: u64 = 42;

    if text.len() > 13 {
        return Err(format!("Free text must be 13 characters or less, got {}", text.len()));
    }

    // Right-align the text with spaces (as per packtext77)
    let padded = format!("{:>13}", text);

    // Encode using base-42: accumulator = accumulator * 42 + char_index
    // We use a big-endian byte array to handle large numbers
    let mut acc = [0u8; 9];  // 71 bits = 9 bytes (7 bits + 8*8 bits)

    for ch in padded.bytes() {
        // Find character index in charset
        let idx = CHARSET_BASE42.iter().position(|&c| c == ch)
            .ok_or_else(|| format!("Invalid character '{}' in free text (valid: {})",
                                   ch as char,
                                   core::str::from_utf8(CHARSET_BASE42).unwrap()))?;

        // Multiply accumulator by 42 and add index
        // acc = acc * 42 + idx
        multiply_add(&mut acc, BASE, idx as u64);
    }

    // Mask the first byte to only use 7 bits (clear MSB)
    acc[0] &= 0x7F;

    Ok(acc)
}

/// Decode 71 bits back to free text (13 characters)
/// Uses base-42 decoding for Type 0 free text messages
pub fn decode_free_text(bits: &[u8; 9]) -> Result<String, String> {
    const BASE: u64 = 42;

    let mut acc = *bits;
    // Ensure first byte only uses 7 bits
    acc[0] &= 0x7F;

    let mut result = String::with_capacity(13);

    // Decode in reverse: extract character by dividing by 42
    for _ in 0..13 {
        let remainder = divide_inplace(&mut acc, BASE);
        if remainder as usize >= CHARSET_BASE42.len() {
            return Err(format!("Invalid base-42 value: remainder {} out of range", remainder));
        }
        result.push(CHARSET_BASE42[remainder as usize] as char);
    }

    // Reverse since we decoded backwards
    Ok(result.chars().rev().collect())
}

/// Multiply a big-endian byte array by a value and add another value
///
/// This implements the accumulator operation: acc = acc * multiplier + addend
/// Used for base-N encoding of large numbers that don't fit in u64.
fn multiply_add(acc: &mut [u8; 9], multiplier: u64, addend: u64) {
    let mut carry = addend;

    // Process from least significant byte to most significant
    for i in (0..9).rev() {
        let val = (acc[i] as u64) * multiplier + carry;
        acc[i] = (val & 0xFF) as u8;
        carry = val >> 8;
    }
}

/// Divide a big-endian byte array by a value in place, returning the remainder
///
/// This implements: (quotient, remainder) = acc / divisor
/// The quotient is stored back in acc, and the remainder is returned.
/// Used for base-N decoding of large numbers.
fn divide_inplace(acc: &mut [u8; 9], divisor: u64) -> u64 {
    let mut remainder = 0u64;

    // Process from most significant byte to least significant
    for i in 0..9 {
        let val = (remainder << 8) | (acc[i] as u64);
        acc[i] = (val / divisor) as u8;
        remainder = val % divisor;
    }

    remainder
}

#[cfg(test)]
mod tests {
    use super::*;

    // Base-38 encoding tests (Type 4 NonStandardCall)
    #[test]
    fn test_base38_simple_callsign() {
        let callsign = "K1ABC";
        let encoded = encode_callsign_base38(callsign).unwrap();
        let decoded = decode_callsign_base38(encoded).unwrap();
        assert_eq!(decoded, callsign);
    }

    #[test]
    fn test_base38_compound_callsign() {
        let callsign = "PJ4/K1ABC";
        let encoded = encode_callsign_base38(callsign).unwrap();
        let decoded = decode_callsign_base38(encoded).unwrap();
        assert_eq!(decoded, callsign);
    }

    #[test]
    fn test_base38_slash_callsign() {
        let callsign = "KH1/KH7Z";
        let encoded = encode_callsign_base38(callsign).unwrap();
        let decoded = decode_callsign_base38(encoded).unwrap();
        assert_eq!(decoded, callsign);
    }

    #[test]
    fn test_base38_max_length() {
        let callsign = "12345678901"; // 11 characters
        let encoded = encode_callsign_base38(callsign).unwrap();
        let decoded = decode_callsign_base38(encoded).unwrap();
        assert_eq!(decoded, callsign);
    }

    #[test]
    fn test_base38_too_long() {
        let callsign = "123456789012"; // 12 characters
        assert!(encode_callsign_base38(callsign).is_err());
    }

    #[test]
    fn test_base38_invalid_char() {
        let callsign = "K1ABC+"; // '+' not in base-38 charset
        assert!(encode_callsign_base38(callsign).is_err());
    }

    #[test]
    fn test_base38_case_insensitive() {
        let lower = "pj4/k1abc";
        let upper = "PJ4/K1ABC";
        let encoded_lower = encode_callsign_base38(lower).unwrap();
        let encoded_upper = encode_callsign_base38(upper).unwrap();
        assert_eq!(encoded_lower, encoded_upper);
    }

    // Base-42 encoding tests (Type 0 free text)
    #[test]
    fn test_base42_free_text() {
        let text = "TNX BOB 73 GL";
        let encoded = encode_free_text(text).unwrap();
        let decoded = decode_free_text(&encoded).unwrap();
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_base42_with_special_chars() {
        let text = "TEST+123-4.5/";
        let encoded = encode_free_text(text).unwrap();
        let decoded = decode_free_text(&encoded).unwrap();
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_base42_max_length() {
        let text = "1234567890ABC"; // 13 characters
        let encoded = encode_free_text(text).unwrap();
        let decoded = decode_free_text(&encoded).unwrap();
        assert_eq!(decoded, text);
    }

    #[test]
    fn test_base42_too_long() {
        let text = "12345678901234"; // 14 characters
        assert!(encode_free_text(text).is_err());
    }

    #[test]
    fn test_base42_invalid_char() {
        let text = "TEST@123"; // '@' not in base-42 charset
        assert!(encode_free_text(text).is_err());
    }

    #[test]
    fn test_base42_short_text() {
        let text = "HI";
        let encoded = encode_free_text(text).unwrap();
        let decoded = decode_free_text(&encoded).unwrap();
        // Should be right-padded with spaces to 13 chars
        assert_eq!(decoded, "           HI");
    }

    #[test]
    fn test_base42_empty() {
        let text = "";
        let encoded = encode_free_text(text).unwrap();
        let decoded = decode_free_text(&encoded).unwrap();
        // Should be 13 spaces
        assert_eq!(decoded, "             ");
    }

    // Roundtrip tests
    #[test]
    fn test_base38_roundtrip_various() {
        let callsigns = vec![
            "W9XYZ",
            "K1ABC",
            "PJ4/K1ABC",
            "KH1/KH7Z",
            "YW18FIFA",
            "G4ABC/P",
            "N0YPR",
        ];

        for callsign in callsigns {
            let encoded = encode_callsign_base38(callsign).unwrap();
            let decoded = decode_callsign_base38(encoded).unwrap();
            assert_eq!(decoded, callsign, "Failed roundtrip for '{}'", callsign);
        }
    }

    #[test]
    fn test_base42_roundtrip_various() {
        let texts = vec![
            "HELLO",
            "TNX BOB 73 GL",
            "CQ DX",
            "73",
            "TEST+123",
            "A/B-C.D?",
        ];

        for text in texts {
            let encoded = encode_free_text(text).unwrap();
            let decoded = decode_free_text(&encoded).unwrap();
            // Account for right-padding
            let expected = format!("{:>13}", text);
            assert_eq!(decoded, expected, "Failed roundtrip for '{}'", text);
        }
    }
}
