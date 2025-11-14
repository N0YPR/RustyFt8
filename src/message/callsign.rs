/// Callsign encoding and decoding functions
///
/// Implements the WSJT-X pack28/unpack28 algorithm for encoding callsigns
/// into 28-bit integers.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use crate::message::constants::{NTOKENS, MAX22, CHARSET_A1, CHARSET_A2, CHARSET_A3, CHARSET_A4, CHARSET_BASE38};

/// Unpack a 28-bit value to a callsign using WSJT-X protocol
///
/// This implements the WSJT-X unpack28 algorithm, reversing pack28 encoding
/// to recover the original callsign from its 28-bit representation.
pub fn unpack_callsign(n28: u32) -> Result<String, String> {
    // Special token handling
    if n28 == 0 {
        return Ok("DE".to_string());
    }
    if n28 == 1 {
        return Ok("QRZ".to_string());
    }
    if n28 == 2 {
        return Ok("CQ".to_string());
    }
    
    // Directed CQ handling (values 3-532443)
    if n28 >= 3 && n28 < NTOKENS {
        // Numeric directed CQ: 3-1002 (CQ 000 - CQ 999)
        if n28 <= 1002 {
            let num = n28 - 3;
            return Ok(format!("CQ {:03}", num));
        }
        
        // Alphabetic directed CQ: 1003+ (CQ A - CQ ZZZZ)
        let value = n28 - 1003;
        
        // Decode from base-27 (space=0, A=1, B=2, ..., Z=26)
        // Try to determine the length by checking ranges
        let mut chars = Vec::new();
        let mut remaining = value;
        
        // Single letter: 1-26
        if value <= 26 {
            let ch = char::from_u32('A' as u32 + value - 1).unwrap();
            return Ok(format!("CQ {}", ch));
        }
        
        // Determine length by checking ranges
        // 2 letters: 27-728 (AA=27+1, ZZ=27+26*27+26)
        // 3 letters: 729-19682 (AAA, ZZZ)
        // 4 letters: 19683+ (AAAA, ZZZZ)
        
        let max_2letter = 27 + 27 * 26;  // 729
        let max_3letter = max_2letter + 27 * 27 * 26;  // 19683
        
        let len = if value < max_2letter {
            2
        } else if value < max_3letter {
            3
        } else {
            4
        };
        
        // Decode each character
        for i in (0..len).rev() {
            let divisor = 27u32.pow(i);
            let idx = remaining / divisor;
            remaining %= divisor;
            
            if idx == 0 {
                chars.push(' ');
            } else if idx <= 26 {
                chars.push(char::from_u32('A' as u32 + idx - 1).unwrap());
            } else {
                return Err(format!("Invalid directed CQ value: {}", n28));
            }
        }
        
        let suffix: String = chars.iter().collect();
        return Ok(format!("CQ {}", suffix.trim_start()));
    }
    
    // Standard callsign decoding
    if n28 >= NTOKENS + MAX22 {
        let n = n28 - NTOKENS - MAX22;

        // Reverse the encoding formula:
        // n = 36*10*27*27*27*i1 + 10*27*27*27*i2 + 27*27*27*i3 + 27*27*i4 + 27*i5 + i6

        let base = 36 * 10 * 27 * 27 * 27;
        let i1 = (n / base) as usize;
        let mut remainder = n % base;

        let base = 10 * 27 * 27 * 27;
        let i2 = (remainder / base) as usize;
        remainder %= base;

        let base = 27 * 27 * 27;
        let i3 = (remainder / base) as usize;
        remainder %= base;

        let base = 27 * 27;
        let i4 = (remainder / base) as usize;
        remainder %= base;

        let i5 = (remainder / 27) as usize;
        let i6 = (remainder % 27) as usize;

        // Validate indices
        if i1 >= CHARSET_A1.len() || i2 >= CHARSET_A2.len() || i3 >= CHARSET_A3.len() ||
           i4 >= CHARSET_A4.len() || i5 >= CHARSET_A4.len() || i6 >= CHARSET_A4.len() {
            return Err(format!("Invalid n28 value produces out-of-range indices: {}", n28));
        }

        // Extract characters
        let c1 = CHARSET_A1.chars().nth(i1).unwrap();
        let c2 = CHARSET_A2.chars().nth(i2).unwrap();
        let c3 = CHARSET_A3.chars().nth(i3).unwrap();
        let c4 = CHARSET_A4.chars().nth(i4).unwrap();
        let c5 = CHARSET_A4.chars().nth(i5).unwrap();
        let c6 = CHARSET_A4.chars().nth(i6).unwrap();
        
        // Build the 6-character callsign and trim leading/trailing spaces
        let callsign_6 = format!("{}{}{}{}{}{}", c1, c2, c3, c4, c5, c6);
        let callsign = callsign_6.trim();
        
        return Ok(callsign.to_string());
    }
    
    // Hash callsign range (n28 >= NTOKENS && n28 < NTOKENS + MAX22)
    // These are non-standard callsigns that were encoded with a hash
    // We cannot recover the original callsign from the hash
    if n28 >= NTOKENS && n28 < NTOKENS + MAX22 {
        return Ok("<...>".to_string());
    }
    
    Err(format!("Invalid n28 value: {} (unexpected hash range value)", n28))
}

/// Calculate a hash of a callsign with specified bit width
/// 
/// Implements the ihashcall function from WSJT-X packjt77.f90:
/// - Converts callsign to base-38 number using characters ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/'
/// - Multiplies by 47055833459
/// - Shifts right to get lower m bits
/// 
/// # Arguments
/// * `callsign` - The callsign to hash
/// * `m` - Number of bits in the hash (typically 10, 12, or 22)
/// 
/// # Returns
/// m-bit hash value
fn ihashcall(callsign: &str, m: u32) -> u32 {
    // Pad callsign to 11 characters (WSJT-X uses 13, but only first 11 matter)
    let mut c13 = callsign.to_uppercase();
    while c13.len() < 11 {
        c13.push(' ');
    }
    if c13.len() > 11 {
        c13.truncate(11);
    }

    // Convert to base-38 number
    let mut n8: u64 = 0;
    for ch in c13.chars() {
        // In Fortran, index() returns 0 if not found (1-indexed), then we subtract 1
        // So not found -> 0 - 1 = -1, but we treat as 0
        // In our case, we find the character position in the base-38 charset
        let j = CHARSET_BASE38.iter().position(|&c| c == ch as u8).unwrap_or(0) as u64;
        n8 = 38 * n8 + j;
    }
    
    // Multiply by 47055833459 and shift to get m bits
    let result = n8.wrapping_mul(47055833459u64);
    let shifted = result >> (64 - m);
    
    (shifted & ((1u64 << m) - 1)) as u32
}

/// Pack a callsign into a 28-bit value using WSJT-X protocol
///
/// This implements the WSJT-X pack28 algorithm from packjt77.f90.
///
/// # Format
/// - Callsign must be right-adjusted to 6 characters
/// - Character sets:
///   - a1: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ' (37 chars, space + 0-9 + A-Z)
///   - a2: '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ' (36 chars, 0-9 + A-Z)
///   - a3: '0123456789' (10 chars)
///   - a4: ' ABCDEFGHIJKLMNOPQRSTUVWXYZ' (27 chars, space + A-Z)
/// - Encoding: n28 = NTOKENS + MAX22 + 36*10*27*27*27*i1 + 10*27*27*27*i2
///                   + 27*27*27*i3 + 27*27*i4 + 27*i5 + i6
pub fn pack_callsign(callsign: &str) -> Result<u32, String> {
    // Special token handling
    if callsign == "DE" {
        return Ok(0);
    }
    if callsign == "QRZ" {
        return Ok(1);
    }
    if callsign == "CQ" {
        return Ok(2);
    }
    
    // Directed CQ handling: "CQ nnn" or "CQ A" etc.
    // Based on WSJT-X pack28 implementation for directed CQ
    if callsign.starts_with("CQ ") {
        let suffix = &callsign[3..];
        
        // Check if it's all numeric (000-999)
        if suffix.chars().all(|c| c.is_ascii_digit()) {
            let n = suffix.parse::<u32>()
                .map_err(|_| format!("Invalid numeric suffix in directed CQ: '{}'", callsign))?;
            if n > 999 {
                return Err(format!("Numeric suffix must be 0-999: '{}'", callsign));
            }
            return Ok(3 + n);
        }
        
        // Check if it's alphabetic (A-ZZZZ)
        if suffix.chars().all(|c| c.is_ascii_alphabetic()) {
            let upper_suffix = suffix.to_uppercase();
            let len = upper_suffix.len();
            
            if len < 1 || len > 4 {
                return Err(format!("Alphabetic suffix must be 1-4 letters: '{}'", callsign));
            }
            
            // Encode using base-27 system (space + A-Z)
            // Space=0, A=1, B=2, ..., Z=26
            let chars: Vec<char> = upper_suffix.chars().collect();
            let mut value = 0u32;
            
            for (i, &ch) in chars.iter().enumerate() {
                if !ch.is_ascii_alphabetic() {
                    return Err(format!("Invalid character in directed CQ suffix: '{}'", ch));
                }
                // A=1, B=2, ..., Z=26 (base-27 with space=0)
                let idx = (ch as u32) - ('A' as u32) + 1;
                value += idx * 27u32.pow((len - 1 - i) as u32);
            }
            
            // Base offset depends on length
            // Pattern from ft8code:
            // - Single letter (A-Z): value + 1003
            // - Two letters (AA-ZZ): value + 1003
            // - Three letters (AAA-ZZZ): value + 1003
            // - Four letters (AAAA-ZZZZ): value + 1003
            let base_offset = 1003;
            
            return Ok(base_offset + value);
        }
        
        return Err(format!("Invalid directed CQ format: '{}' (suffix must be numeric 000-999 or alphabetic A-ZZZZ)", callsign));
    }
    
    // Non-standard callsign handling (angle brackets)
    // Format: <CALLSIGN> (e.g., "<KH1/KH7Z>", "<PJ4/K1ABC>")
    // These are encoded using a 22-bit hash: n28 = NTOKENS + hash22(callsign)
    if callsign.starts_with('<') && callsign.ends_with('>') {
        let inner = &callsign[1..callsign.len()-1];
        let hash22_value = hash22(inner);
        return Ok(NTOKENS + hash22_value);
    }
    
    // Slash callsign handling
    // Format: PREFIX/CALL or CALL/SUFFIX
    // - PREFIX/CALL (e.g., "KH1/KH7Z", "W1/K1ABC"): Strip prefix, encode base call
    // - CALL/P or CALL/R: Strip suffix, encode base call (the /P or /R flag is handled by message encoder)
    let mut base_call = callsign;
    
    if callsign.contains('/') {
        let parts: Vec<&str> = callsign.split('/').collect();
        if parts.len() == 2 {
            // Check if it's CALL/P or CALL/R (portable/rover suffix)
            if parts[1] == "P" || parts[1] == "R" || parts[1] == "p" || parts[1] == "r" {
                // Use the base call before the slash
                base_call = parts[0];
            } else {
                // It's PREFIX/CALL format, use the part after the slash
                base_call = parts[1];
            }
        } else {
            return Err(format!("Invalid slash callsign format: '{}'", callsign));
        }
    }
    
    // For now, only implement standard callsign encoding
    // Standard callsign must:
    // - Have 3-6 characters
    // - Have exactly one digit (the area number)
    // - Have the digit in position 2 or 3
    // - Have 1-2 letters before the digit
    // - Have 1-3 letters after the digit
    
    let call = base_call.to_uppercase();
    let chars: Vec<char> = call.chars().collect();
    let n = chars.len();
    
    if n < 3 || n > 6 {
        return Err(format!("Callsign length must be 3-6 characters: '{}' (from '{}')", base_call, callsign));
    }
    
    // Find the area digit position
    let mut iarea = None;
    for i in (1..n).rev() {
        if chars[i].is_ascii_digit() {
            iarea = Some(i);
            break;
        }
    }
    
    let iarea = match iarea {
        Some(pos) if pos >= 1 && pos <= 2 => pos,
        _ => return Err(format!("Invalid callsign format: '{}' (must have a digit in position 2 or 3)", callsign)),
    };
    
    // Count digits and letters before area digit
    let mut npdig = 0;
    let mut nplet = 0;
    for i in 0..iarea {
        if chars[i].is_ascii_digit() {
            npdig += 1;
        }
        if chars[i].is_ascii_alphabetic() {
            nplet += 1;
        }
    }
    
    // Count letters after area digit
    let mut nslet = 0;
    for i in (iarea + 1)..n {
        if chars[i].is_ascii_alphabetic() {
            nslet += 1;
        }
    }
    
    // Validate standard callsign format
    if nplet == 0 || npdig >= iarea || nslet > 3 {
        return Err(format!("Invalid standard callsign format: '{}' (must have 1-2 letters before digit, max 3 letters after)", callsign));
    }
    
    // Right-adjust to 6 characters with space padding on the left
    let callsign_6 = if iarea == 1 {
        format!(" {:<5}", call)
    } else {
        format!("{:<6}", call)
    };
    
    let c6: Vec<char> = callsign_6.chars().collect();

    // Find indices in character sets
    let i1 = CHARSET_A1.find(c6[0]).ok_or_else(|| format!("Invalid character at position 1: '{}'", c6[0]))?;
    let i2 = CHARSET_A2.find(c6[1]).ok_or_else(|| format!("Invalid character at position 2: '{}'", c6[1]))?;
    let i3 = CHARSET_A3.find(c6[2]).ok_or_else(|| format!("Invalid character at position 3: '{}'", c6[2]))?;
    let i4 = CHARSET_A4.find(c6[3]).ok_or_else(|| format!("Invalid character at position 4: '{}'", c6[3]))?;
    let i5 = CHARSET_A4.find(c6[4]).ok_or_else(|| format!("Invalid character at position 5: '{}'", c6[4]))?;
    let i6 = CHARSET_A4.find(c6[5]).ok_or_else(|| format!("Invalid character at position 6: '{}'", c6[5]))?;
    
    // Encode as per WSJT-X formula
    let n28 = 36 * 10 * 27 * 27 * 27 * (i1 as u32)
            + 10 * 27 * 27 * 27 * (i2 as u32)
            + 27 * 27 * 27 * (i3 as u32)
            + 27 * 27 * (i4 as u32)
            + 27 * (i5 as u32)
            + (i6 as u32)
            + NTOKENS
            + MAX22;
    
    // Mask to 28 bits
    Ok(n28 & ((1 << 28) - 1))
}

/// Compute a 10-bit hash of a callsign
///
/// Used in Type 0.5 messages (DXpedition mode) for referencing callsigns.
///
/// # Arguments
/// * `callsign` - The callsign to hash
///
/// # Returns
/// 10-bit hash value
pub fn hash10(callsign: &str) -> u16 {
    ihashcall(callsign, 10) as u16
}

/// Compute a 12-bit hash of a callsign
///
/// Used in Type 2 messages for non-standard callsigns.
///
/// # Arguments
/// * `callsign` - The callsign to hash
///
/// # Returns
/// 12-bit hash value
pub fn hash12(callsign: &str) -> u16 {
    ihashcall(callsign, 12) as u16
}

/// Compute a 22-bit hash of a callsign
///
/// Used in Type 1 messages when referencing non-standard callsigns.
///
/// # Arguments
/// * `callsign` - The callsign to hash
///
/// # Returns
/// 22-bit hash value
pub fn hash22(callsign: &str) -> u32 {
    ihashcall(callsign, 22)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn test_invalid_callsigns() {
        // No digits
        assert!(pack_callsign("ABC").is_err());
        // Too long
        assert!(pack_callsign("AB1CDEF").is_err());
        // Too many letters after digit (more than 3)
        assert!(pack_callsign("A1BCDE").is_err());
    }

    /// Test special callsigns that are too long for standard pack28 encoding
    /// These would need angle brackets and hash encoding: <WB2000XYZ>
    #[test]
    fn test_unsupported_callsign_formats() {
        let unsupported = vec![
            "WB2000XYZ",         // Too long (9 chars) - would need <WB2000XYZ>
            "WB2000XYZABCD",     // Too long (13 chars) - would need <WB2000XYZABCD>
        ];
        
        for call in unsupported {
            let result = pack_callsign(call);
            // These should fail because they're too long for standard encoding
            // They would need angle brackets to use hash encoding: <WB2000XYZ>
            assert!(result.is_err(), "Should fail without angle brackets: {}", call);
        }

        // Verify they work with angle brackets
        assert!(pack_callsign("<WB2000XYZ>").is_ok());
        assert!(pack_callsign("<WB2000XYZABCD>").is_ok());
    }

    /// Test callsign encoding and decoding against ft8code reference values
    /// Each test case verifies:
    /// 1. encode(callsign) produces the expected n28 value
    /// 2. decode(n28) produces the expected callsign
    /// 3. Round-trip: decode(encode(callsign)) == callsign (uppercase)
    #[rstest]
    // Special tokens
    #[case::cq("CQ", 0b0000000000000000000000000010)]
    #[case::de("DE", 0b0000000000000000000000000000)]
    #[case::qrz("QRZ", 0b0000000000000000000000000001)]
    // Directed CQ - Numeric suffixes (000-999)
    #[case::cq_000("CQ 000", 0b0000000000000000000000000011)]
    #[case::cq_001("CQ 001", 0b0000000000000000000000000100)]
    #[case::cq_313("CQ 313", 0b0000000000000000000100111100)]
    #[case::cq_999("CQ 999", 0b0000000000000000001111101010)]
    // Directed CQ - Single letter suffixes (A-Z)
    #[case::cq_a("CQ A", 0b0000000000000000001111101100)]
    #[case::cq_z("CQ Z", 0b0000000000000000010000000101)]
    // Directed CQ - Two letter suffixes (AA-ZZ)
    #[case::cq_aa("CQ AA", 0b0000000000000000010000000111)]
    #[case::cq_ab("CQ AB", 0b0000000000000000010000001000)]
    #[case::cq_ba("CQ BA", 0b0000000000000000010000100010)]
    #[case::cq_dx("CQ DX", 0b0000000000000000010001101111)]
    #[case::cq_zz("CQ ZZ", 0b0000000000000000011011000011)]
    // Directed CQ - Three/Four letter suffixes
    #[case::cq_aaa("CQ AAA", 0b0000000000000000011011100000)]
    #[case::cq_abc("CQ ABC", 0b0000000000000000011011111101)]
    #[case::cq_abcd("CQ ABCD", 0b0000000000000101011011010101)]
    #[case::cq_zzzz("CQ SOTA", 0b0000000001011110010110011000)]
    #[case::cq_zzzz("CQ ZZZZ", 0b0000000010000001111111011011)]
    // Non-standard callsigns (hashed, angle brackets)
    #[case::nonstandard_kh1_kh7z("<KH1/KH7Z>", 0b0000001011000001011010110101)]  // hash22 = 825805
    #[case::nonstandard_pj4_k1abc("<PJ4/K1ABC>", 0b0000001101010010101100001010)]  // hash22 = 1420834
    #[case::nonstandard_w9xyz_7("<W9XYZ/7>", 0b0000001111011001101010010010)]  // hash22 = 1973674
    #[case::nonstandard_3d2ag("<3D2AG>", 0b0000010110110111100101001100)]  // hash22 = 3931236
    // Slash callsigns (note: slash info stripped, base call encoded)
    #[case::kh1_kh7z("KH7Z", 0b1001011100110111111101111101)]  // KH1/KH7Z encodes as KH7Z
    #[case::w1_k1abc("K1ABC", 0b0000100110111101111000110101)]  // W1/K1ABC encodes as K1ABC
    #[case::vp2e_ka1abc("KA1ABC", 0b1001010111000110010100100001)]  // VP2E/KA1ABC encodes as KA1ABC
    // Common US callsigns
    #[case::n0ypr("N0YPR", 0b0000101001001101100111001101)]
    #[case::n0ypr_lower("n0ypr", 0b0000101001001101100111001101)]  // Case-insensitive
    #[case::k1jt("K1JT", 0b0000100110111111100110111001)]
    #[case::k1jt_lower("k1jt", 0b0000100110111111100110111001)]  // Case-insensitive
    #[case::w1abc("W1ABC", 0b0000101111111110100010011101)]
    #[case::k9abc("K9ABC", 0b0000100111100100010101001101)]
    #[case::k1abc("K1ABC", 0b0000100110111101111000110101)]
    #[case::ka1abc("KA1ABC", 0b1001010111000110010100100001)]
    #[case::ka1jt("KA1JT", 0b1001010111001000000010100101)]
    #[case::ka0abc("KA0ABC", 0b1001010111000001100000111110)]
    #[case::wb9xyz("WB9XYZ", 0b1110011100111000011110111010)]
    // Minimal callsigns (3-4 characters)
    #[case::a0a("A0A", 0b0000011111011000100001101101)]
    #[case::a0aa("A0AA", 0b0000011111011000100010001000)]
    #[case::a0aab("A0AAB", 0b0000011111011000100010001010)]
    #[case::a0aaa("A0AAA", 0b0000011111011000100010001001)]
    #[case::a00a("A00A", 0b0101000001001101011100101001)]
    // Full 6-character callsign
    #[case::aa0aaa("AA0AAA", 0b0101001000101101111111110001)]
    // International callsigns
    #[case::call_5b1abc("5B1ABC", 0b0011000010011001000110110111)]
    #[case::call_9y4xyz("9Y4XYZ", 0b0101000000000100110100110101)]
    #[case::call_9y4ab("9Y4AB", 0b0101000000000000100100101111)]
    #[case::ve3abc("VE3ABC", 0b1110000011100101100111000111)]
    #[case::g4abc("G4ABC", 0b0000100100001100000101100110)]
    #[case::ja1abc("JA1ABC", 0b1000111100000100010111101001)]
    #[case::w9xyz("W9XYZ", 0b0000110000101001001110111000)]
    #[case::kk7jxp("KK7JXP", 0b1001011111000101011100011111)]
    #[case::pa9xyz("PA9XYZ", 0b1011011110111010110001010100)]
    #[case::g3aaa("G3AAA", 0b0000100100000111010001100110)]
    fn test_callsign_encode_decode(#[case] callsign: &str, #[case] expected_n28: u32) {
        // Test encoding
        let encoded = pack_callsign(callsign)
            .expect(&format!("Failed to encode callsign: {}", callsign));

        assert_eq!(
            encoded, expected_n28,
            "Encoding '{}' mismatch:\n  Expected: {}\n  Actual:   {}",
            callsign, expected_n28, encoded
        );

        // For hashed callsigns (non-standard with angle brackets), we cannot recover
        // the original callsign from the hash. The decoder returns "<...>" for these.
        let is_hashed = callsign.starts_with('<') && callsign.ends_with('>');

        if !is_hashed {
            // Test decoding (only for non-hashed callsigns)
            let decoded = unpack_callsign(expected_n28)
                .expect(&format!("Failed to decode n28: {}", expected_n28));

            assert_eq!(
                decoded, callsign.to_uppercase(),
                "Decoding {} mismatch:\n  Expected: {}\n  Actual:   {}",
                expected_n28, callsign.to_uppercase(), decoded
            );

            // Test round-trip
            let roundtrip = unpack_callsign(encoded)
                .expect(&format!("Failed to decode encoded value: {}", encoded));

            assert_eq!(
                roundtrip, callsign.to_uppercase(),
                "Round-trip failed for '{}': encode = {} -> decode = '{}'",
                callsign, encoded, roundtrip
            );
        } else {
            // For hashed callsigns, verify that decode returns the placeholder
            let decoded = unpack_callsign(expected_n28)
                .expect(&format!("Failed to decode hashed n28: {}", expected_n28));

            assert_eq!(
                decoded, "<...>",
                "Hashed callsign should decode to '<...>', got: '{}'",
                decoded
            );
        }
    }

    /// Test slash callsigns encode only the base call
    #[rstest]
    #[case("KH1/KH7Z", "KH7Z", 158564221)]  // Prefix stripped
    #[case("W1/K1ABC", "K1ABC", 10214965)]   // Prefix stripped
    #[case("VP2E/KA1ABC", "KA1ABC", 157050145)]  // Prefix stripped
    #[case("K1ABC/P", "K1ABC", 10214965)]    // Portable suffix stripped
    #[case("N0YPR/R", "N0YPR", 10803661)]    // Rover suffix stripped
    fn test_slash_callsigns(#[case] slash_call: &str, #[case] base_call: &str, #[case] expected_n28: u32) {
        let encoded = pack_callsign(slash_call).unwrap();
        assert_eq!(encoded, expected_n28);

        // Decode returns the base call (without slash)
        let decoded = unpack_callsign(encoded).unwrap();
        assert_eq!(decoded, base_call);
    }
}
