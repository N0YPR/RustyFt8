//! CRC-14 Implementation for FT8
//!
//! This module implements the 14-bit CRC used in FT8 message encoding.
//! The CRC polynomial is 0x2757.
//!
//! Reference: https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf page 8
//! "The CRC is calculated on the source-encoded message, zero-extended from 77 to 82 bits."

use crc::{Algorithm, Crc};
use bitvec::prelude::*;

/// FT8 CRC-14 polynomial
const CRC_POLYNOMIAL: u16 = 0x2757;

/// FT8 CRC-14 algorithm configuration
const CRC_FT8: Algorithm<u16> = Algorithm {
    width: 14,
    poly: CRC_POLYNOMIAL,
    init: 0x0,
    refin: false,
    refout: false,
    xorout: 0x0,
    check: 0x0,
    residue: 0x0,
};

/// FT8 CRC instance
const FT8_CRC: Crc<u16> = Crc::<u16>::new(&CRC_FT8);

/// Calculate 14-bit CRC for FT8 messages
///
/// This function computes the CRC-14 for a 77-bit message by:
/// 1. Zero-extending the message from 77 to 82 bits (adding 5 zero bits)
/// 2. Converting to bytes
/// 3. Computing CRC-14 using polynomial 0x2757
///
/// # Arguments
/// * `bits` - Bit slice containing the 77-bit message
///
/// # Returns
/// * `u16` - 14-bit CRC value (only lower 14 bits are valid)
///
/// # Example
/// ```
/// use bitvec::prelude::*;
/// use rustyft8::crc::crc14;
///
/// // Example: 77-bit message
/// let bits = bitarr![u8, Msb0; 0; 77];
/// let crc = crc14(&bits);
/// assert!(crc < (1 << 14)); // CRC is 14 bits
/// ```
pub fn crc14(bits: &BitSlice<u8, Msb0>) -> u16 {
    // Convert bits to u128 (we only use lower 77 bits)
    let mut msg: u128 = 0;
    for (i, bit) in bits.iter().take(77).enumerate() {
        if *bit {
            msg |= 1u128 << (76 - i);
        }
    }

    // Zero-extend from 77 to 82 bits by left-shifting by 5
    let padded_msg = msg << 5;

    // Convert to big-endian bytes
    let msg_bytes = padded_msg.to_be_bytes();

    // Only need last 11 bytes (88 bits, but we only use 82)
    let trimmed_bytes = &msg_bytes[msg_bytes.len() - 11..];

    // Calculate CRC
    FT8_CRC.checksum(trimmed_bytes)
}

/// Check if a message with CRC is valid
///
/// # Arguments
/// * `bits` - Bit slice containing 91-bit message (77 message + 14 CRC)
///
/// # Returns
/// * `bool` - true if CRC is valid
pub fn crc14_check(bits: &BitSlice<u8, Msb0>) -> bool {
    // Calculate CRC for first 77 bits
    let calculated_crc = crc14(&bits[..77]);

    // Extract CRC from bits 77-90
    let mut received_crc: u16 = 0;
    for bit in bits[77..91].iter() {
        received_crc = (received_crc << 1) | (*bit as u16);
    }

    calculated_crc == received_crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc14_zero_message() {
        // All zeros should produce a specific CRC
        let bits = bitarr![u8, Msb0; 0; 77];
        let crc = crc14(&bits);
        // Zero message gives zero CRC
        assert_eq!(crc, 0);
    }

    #[test]
    fn test_crc14_simple_pattern() {
        // Test with a simple pattern
        let mut bits = bitarr![u8, Msb0; 0; 77];
        for i in 0..8 {
            bits.set(i, true);
        }
        let crc = crc14(&bits);
        // CRC should be non-zero for non-zero input
        assert_ne!(crc, 0);
        assert!(crc < (1 << 14)); // Must be 14 bits
    }

    #[test]
    fn test_crc14_check_valid() {
        // Create a message with valid CRC
        let mut bits = bitarr![u8, Msb0; 0; 91];
        bits.set(0, true);
        bits.set(10, true);

        // Calculate CRC for the message
        let crc = crc14(&bits[..77]);

        // Store CRC in last 14 bits
        for i in 0..14 {
            bits.set(77 + i, ((crc >> (13 - i)) & 1) != 0);
        }

        // Check should pass
        assert!(crc14_check(&bits));
    }

    #[test]
    fn test_crc14_bounds() {
        // CRC should always be 14 bits (< 16384)
        for pattern in 0..16u8 {
            let mut bits = bitarr![u8, Msb0; 0; 77];
            for i in 0..77 {
                bits.set(i, ((pattern >> (i % 4)) & 1) != 0);
            }
            let crc = crc14(&bits);
            assert!(crc < 16384, "CRC {} exceeds 14 bits", crc);
        }
    }

    #[test]
    fn test_crc14_known_message() {
        // Test against known message from WSJT-X ft8code
        // Message: "CQ SOTA N0YPR/R DM42"
        // 77-bit message: 00000000010111100101100110000000010100100110110011100110110001100111110010001
        // Expected CRC: 00001001100101 (binary) = 0b00001001100101

        let bits_str = "00000000010111100101100110000000010100100110110011100110110001100111110010001";
        let mut bits = bitarr![u8, Msb0; 0; 77];

        // Convert string to bit array
        for (i, c) in bits_str.chars().enumerate() {
            bits.set(i, c == '1');
        }

        let crc = crc14(&bits);
        assert_eq!(crc, 0b00001001100101, "CRC mismatch: got {}, expected {}", crc, 0b00001001100101);
    }
}
