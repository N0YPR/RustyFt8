//! LDPC (Low-Density Parity Check) Error Correction for FT8
//!
//! This module implements the LDPC(174,91) encoding used in FT8.
//! It takes a 91-bit message (77 information bits + 14 CRC bits) and
//! produces a 174-bit codeword by adding 83 parity bits.
//!
//! The encoding uses a generator matrix to compute parity bits through
//! matrix multiplication in GF(2) (binary field).

use bitvec::prelude::*;

/// LDPC(174,91) parameters
const N: usize = 174; // Codeword length
const K: usize = 91;  // Message length (77 + 14 CRC)
const M: usize = 83;  // Parity bits (N - K)

/// Generator matrix for LDPC(174,91) encoding
///
/// This is an 83×91 matrix stored in compressed hexadecimal format.
/// Each row is represented as 23 hex digits (91 bits + padding).
/// The matrix is derived from the WSJT-X implementation.
const GENERATOR_MATRIX_HEX: [&str; 83] = [
    "8329ce11bf31eaf509f27fc",
    "761c264e25c259335493132",
    "dc265902fb277c6410a1bdc",
    "1b3f417858cd2dd33ec7f62",
    "09fda4fee04195fd034783a",
    "077cccc11b8873ed5c3d48a",
    "29b62afe3ca036f4fe1a9da",
    "6054faf5f35d96d3b0c8c3e",
    "e20798e4310eed27884ae90",
    "775c9c08e80e26ddae56318",
    "b0b811028c2bf997213487c",
    "18a0c9231fc60adf5c5ea32",
    "76471e8302a0721e01b12b8",
    "ffbccb80ca8341fafb47b2e",
    "66a72a158f9325a2bf67170",
    "c4243689fe85b1c51363a18",
    "0dff739414d1a1b34b1c270",
    "15b48830636c8b99894972e",
    "29a89c0d3de81d665489b0e",
    "4f126f37fa51cbe61bd6b94",
    "99c47239d0d97d3c84e0940",
    "1919b75119765621bb4f1e8",
    "09db12d731faee0b86df6b8",
    "488fc33df43fbdeea4eafb4",
    "827423ee40b675f756eb5fe",
    "abe197c484cb74757144a9a",
    "2b500e4bc0ec5a6d2bdbdd0",
    "c474aa53d70218761669360",
    "8eba1a13db3390bd6718cec",
    "753844673a27782cc42012e",
    "06ff83a145c37035a5c1268",
    "3b37417858cc2dd33ec3f62",
    "9a4a5a28ee17ca9c324842c",
    "bc29f465309c977e89610a4",
    "2663ae6ddf8b5ce2bb29488",
    "46f231efe457034c1814418",
    "3fb2ce85abe9b0c72e06fbe",
    "de87481f282c153971a0a2e",
    "fcd7ccf23c69fa99bba1412",
    "f0261447e9490ca8e474cec",
    "4410115818196f95cdd7012",
    "088fc31df4bfbde2a4eafb4",
    "b8fef1b6307729fb0a078c0",
    "5afea7acccb77bbc9d99a90",
    "49a7016ac653f65ecdc9076",
    "1944d085be4e7da8d6cc7d0",
    "251f62adc4032f0ee714002",
    "56471f8702a0721e00b12b8",
    "2b8e4923f2dd51e2d537fa0",
    "6b550a40a66f4755de95c26",
    "a18ad28d4e27fe92a4f6c84",
    "10c2e586388cb82a3d80758",
    "ef34a41817ee02133db2eb0",
    "7e9c0c54325a9c15836e000",
    "3693e572d1fde4cdf079e86",
    "bfb2cec5abe1b0c72e07fbe",
    "7ee18230c583cccc57d4b08",
    "a066cb2fedafc9f52664126",
    "bb23725abc47cc5f4cc4cd2",
    "ded9dba3bee40c59b5609b4",
    "d9a7016ac653e6decdc9036",
    "9ad46aed5f707f280ab5fc4",
    "e5921c77822587316d7d3c2",
    "4f14da8242a8b86dca73352",
    "8b8b507ad467d4441df770e",
    "22831c9cf1169467ad04b68",
    "213b838fe2ae54c38ee7180",
    "5d926b6dd71f085181a4e12",
    "66ab79d4b29ee6e69509e56",
    "958148682d748a38dd68baa",
    "b8ce020cf069c32a723ab14",
    "f4331d6d461607e95752746",
    "6da23ba424b9596133cf9c8",
    "a636bcbc7b30c5fbeae67fe",
    "5cb0d86a07df654a9089a20",
    "f11f106848780fc9ecdd80a",
    "1fbb5364fb8d2c9d730d5ba",
    "fcb86bc70a50c9d02a5d034",
    "a534433029eac15f322e34c",
    "c989d9c7c3d3b8c55d75130",
    "7bb38b2f0186d46643ae962",
    "2644ebadeb44b9467d1f42c",
    "608cc857594bfbb55d69600",
];

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
/// use rustyft8::ldpc::ldpc_encode;
///
/// // Create storage with enough bytes for the bit arrays
/// let mut message_storage = [0u8; 12]; // 96 bits for 91-bit message
/// let message = &message_storage.view_bits::<Msb0>()[..91];
/// // ... fill message with 77 info bits + 14 CRC bits ...
///
/// let mut codeword_storage = [0u8; 22]; // 176 bits for 174-bit codeword
/// let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
/// ldpc_encode(message, codeword);
/// // codeword now contains 174 bits (91 message + 83 parity)
/// ```
pub fn ldpc_encode(message: &BitSlice<u8, Msb0>, codeword: &mut BitSlice<u8, Msb0>) {
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

        ldpc_encode(message, codeword);

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
        ldpc_encode(message, codeword);

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
        ldpc_encode(message, codeword);

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
        let mut message_storage = [0u8; 10]; // 80 bits
        let message = &message_storage.view_bits::<Msb0>()[..77]; // Wrong length
        let mut codeword_storage = [0u8; 22]; // 176 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
        ldpc_encode(message, codeword);
    }

    #[test]
    #[should_panic(expected = "Codeword must be 174 bits")]
    fn test_invalid_codeword_length() {
        let mut message_storage = [0u8; 12]; // 96 bits
        let message = &message_storage.view_bits::<Msb0>()[..91];
        let mut codeword_storage = [0u8; 13]; // 104 bits
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..100]; // Wrong length
        ldpc_encode(message, codeword);
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
        ldpc_encode(message, codeword);

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
