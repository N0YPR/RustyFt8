//! LDPC encoder for FT8
//!
//! This module implements LDPC(174,91) encoding using a generator matrix.

use bitvec::prelude::*;
use super::constants::*;

/// Get a bit from the generator matrix at the given row and column
///
/// This function parses the hexadecimal representation on-the-fly to extract
/// the bit value at the specified position. While this is less efficient than
/// caching the full matrix, it works in no_std environments without requiring
/// unsafe static mut access.
///
/// # Arguments
/// * `row` - Row index (0..83)
/// * `col` - Column index (0..91)
///
/// # Returns
/// * `u8` - The bit value (0 or 1) at the specified position
fn get_generator_bit(row: usize, col: usize) -> u8 {
    if row >= M || col >= K {
        return 0;
    }

    let hex_str = GENERATOR_MATRIX_HEX[row];

    // Each hex digit represents 4 bits
    // Column index maps to hex position and bit position within that hex digit
    let hex_idx = col / 4;
    let bit_pos = col % 4;

    // Handle the last hex digit (position 22) which only has 3 valid bits
    if hex_idx >= hex_str.len() {
        return 0;
    }

    // For the last hex digit (position 22), only bits 0-2 are valid (91 = 22*4 + 3)
    if hex_idx == 22 && bit_pos >= 3 {
        return 0;
    }

    let hex_char = hex_str.as_bytes()[hex_idx] as char;
    let digit = match hex_char {
        '0'..='9' => (hex_char as u8) - b'0',
        'a'..='f' => (hex_char as u8) - b'a' + 10,
        'A'..='F' => (hex_char as u8) - b'A' + 10,
        _ => 0,
    };

    // Extract the bit at the position (MSB first)
    ((digit >> (3 - bit_pos)) & 1) as u8
}

/// Encode a 91-bit message using LDPC(174,91)
///
/// Takes a 91-bit message (77 information bits + 14 CRC bits) and produces
/// a 174-bit codeword by computing 83 parity bits using the generator matrix.
///
/// # Arguments
/// * `message` - 91-bit message as BitSlice
/// * `codeword` - Output buffer for 174-bit codeword
///
/// # Example
/// ```
/// use bitvec::prelude::*;
/// use rustyft8::ldpc;
///
/// // Create storage with enough bytes for the bit arrays
/// let mut message_storage = [0u8; 12]; // 96 bits for 91-bit message
/// let message = &message_storage.view_bits::<Msb0>()[..91];
/// // ... fill message with 77 info bits + 14 CRC bits ...
///
/// let mut codeword_storage = [0u8; 22]; // 176 bits for 174-bit codeword
/// let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
/// ldpc::encode(message, codeword);
/// // codeword now contains 174 bits (91 message + 83 parity)
/// ```
pub fn encode(message: &BitSlice<u8, Msb0>, codeword: &mut BitSlice<u8, Msb0>) {
    assert_eq!(message.len(), K, "Message must be {} bits", K);
    assert_eq!(codeword.len(), N, "Codeword must be {} bits", N);

    // Copy message to first K bits of codeword
    codeword[..K].copy_from_bitslice(message);

    // Compute parity bits using matrix multiplication in GF(2)
    for i in 0..M {
        let mut parity = false;
        for j in 0..K {
            parity ^= message[j] & (get_generator_bit(i, j) != 0);
        }
        codeword.set(K + i, parity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldpc_encode_all_zeros() {
        let mut message_storage = [0u8; 12]; // 96 bits
        let message = &message_storage.view_bits_mut::<Msb0>()[..91];
        let mut codeword_storage = [0u8; 22]; // 176 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];

        encode(message, codeword);

        // All zeros should produce all zero codeword
        assert!(codeword.not_any());
    }

    #[test]
    fn test_ldpc_encode_first_bit_set() {
        let mut message_storage = [0u8; 12]; // 96 bits
        let message = &mut message_storage.view_bits_mut::<Msb0>()[..91];
        message.set(0, true);

        let mut codeword_storage = [0u8; 22]; // 176 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
        encode(message, codeword);

        // First K bits should match message
        assert_eq!(&codeword[..K], &message[..]);

        // Parity bits should match first column of generator matrix
        for i in 0..M {
            assert_eq!(codeword[K + i], get_generator_bit(i, 0) != 0, "Parity bit {} mismatch", i);
        }
    }

    #[test]
    fn test_ldpc_codeword_structure() {
        let mut message_storage = [0u8; 12]; // 96 bits
        let message = &mut message_storage.view_bits_mut::<Msb0>()[..91];
        message.set(10, true);
        message.set(20, true);
        message.set(30, true);

        let mut codeword_storage = [0u8; 22]; // 176 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
        encode(message, codeword);

        // Verify codeword structure
        assert_eq!(codeword.len(), N);

        // First K bits are the message
        assert_eq!(&codeword[..K], &message[..]);

        // Remaining M bits are parity
        // At least some parity bits should be set for non-zero message
        let parity_count = codeword[K..].count_ones();
        assert!(parity_count > 0, "Expected some parity bits to be set");
    }

    #[test]
    fn test_generator_matrix_dimensions() {
        // Verify all entries are 0 or 1 by checking a sample of positions
        for i in 0..M {
            for j in 0..K {
                let bit = get_generator_bit(i, j);
                assert!(bit == 0 || bit == 1, "Matrix entries must be 0 or 1 at ({}, {})", i, j);
            }
        }

        // Test out-of-bounds access returns 0
        assert_eq!(get_generator_bit(M, 0), 0);
        assert_eq!(get_generator_bit(0, K), 0);
    }

    #[test]
    #[should_panic(expected = "Message must be 91 bits")]
    fn test_invalid_message_length() {
        let message_storage = [0u8; 10]; // 80 bits
        let message = &message_storage.view_bits::<Msb0>()[..77]; // Wrong length
        let mut codeword_storage = [0u8; 22]; // 176 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
        encode(message, codeword);
    }

    #[test]
    #[should_panic(expected = "Codeword must be 174 bits")]
    fn test_invalid_codeword_length() {
        let message_storage = [0u8; 12]; // 96 bits
        let message = &message_storage.view_bits::<Msb0>()[..91];
        let mut codeword_storage = [0u8; 13]; // 104 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..100]; // Wrong length
        encode(message, codeword);
    }

    #[test]
    fn test_ldpc_encode_known_message() {
        // Test against known message from WSJT-X ft8code
        // Message: "CQ SOTA N0YPR/R DM42"
        // 77-bit message: 00000000010111100101100110000000010100100110110011100110110001100111110010001
        // 14-bit CRC: 00001001100101
        // 83 parity bits: 11100110011001101100100111100011101000010001100111111001100110001110011001011110010

        // 77-bit message
        let msg_str = "00000000010111100101100110000000010100100110110011100110110001100111110010001";
        let mut message_storage = [0u8; 12]; // 96 bits
        let message = &mut message_storage.view_bits_mut::<Msb0>()[..91];
        for (i, c) in msg_str.chars().enumerate() {
            message.set(i, c == '1');
        }

        // 14-bit CRC
        let crc_str = "00001001100101";
        for (i, c) in crc_str.chars().enumerate() {
            message.set(77 + i, c == '1');
        }

        // Expected 83 parity bits from WSJT-X
        let expected_parity_str = "11100110011001101100100111100011101000010001100111111001100110001110011001011110010";
        let mut expected_parity_storage = [0u8; 11]; // 88 bits
        let expected_parity = &mut expected_parity_storage.view_bits_mut::<Msb0>()[..83];
        for (i, c) in expected_parity_str.chars().enumerate() {
            expected_parity.set(i, c == '1');
        }

        // Encode
        let mut codeword_storage = [0u8; 22]; // 176 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
        encode(message, codeword);

        // Verify message bits are preserved
        assert_eq!(&codeword[..K], &message[..]);

        // Verify parity bits match WSJT-X output
        for i in 0..M {
            assert_eq!(
                codeword[K + i], expected_parity[i],
                "Parity bit {} mismatch: got {}, expected {}",
                i, codeword[K + i], expected_parity[i]
            );
        }
    }
}
