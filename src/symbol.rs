//! FT8 Symbol Mapping and Demapping
//!
//! This module handles the conversion between LDPC codewords (174 bits) and
//! FT8 symbol arrays (79 symbols using 8-FSK modulation).
//!
//! **Symbol Structure**: S7 D29 S7 D29 S7
//! - 3 sync sections of 7 symbols each (Costas arrays) = 21 sync symbols
//! - 2 data sections of 29 symbols each = 58 data symbols
//! - Total: 79 symbols
//!
//! **Mapping**: Each 3 bits from the 174-bit codeword map to one data symbol
//! using Gray coding to minimize bit errors between adjacent tones (0-7).


/// Number of data symbols (174 bits / 3 bits per symbol)
pub const ND: usize = 58;

/// Number of sync symbols (3 Costas arrays × 7 symbols each)
pub const NS: usize = 21;

/// Total number of symbols in FT8 transmission
pub const NN: usize = 79;

/// Costas 7×7 sync pattern
/// Used three times in the FT8 symbol sequence for time and frequency synchronization
const COSTAS: [u8; 7] = [3, 1, 4, 0, 6, 5, 2];

/// Gray code mapping for 3-bit values to tones (0-7)
/// This mapping minimizes bit errors between adjacent tones in 8-FSK modulation
const GRAY_MAP: [u8; 8] = [0, 1, 3, 2, 5, 6, 4, 7];

/// Inverse Gray code mapping (tone to 3-bit value)
/// Used for demapping received symbols back to bits
const GRAY_MAP_INV: [u8; 8] = [0, 1, 3, 2, 6, 4, 5, 7];

/// Map a 174-bit LDPC codeword to 79 FT8 symbols
///
/// Converts the codeword into 58 data symbols using Gray coding and inserts
/// 21 sync symbols (Costas arrays) at the proper positions.
///
/// # Structure
/// The resulting 79-symbol array follows the pattern: S7 D29 S7 D29 S7
/// - Positions 0-6: First Costas sync (7 symbols)
/// - Positions 7-35: First data block (29 symbols)
/// - Positions 36-42: Second Costas sync (7 symbols)
/// - Positions 43-71: Second data block (29 symbols)
/// - Positions 72-78: Third Costas sync (7 symbols)
///
/// # Arguments
/// * `codeword` - 174-bit LDPC codeword (bit slice)
/// * `symbols` - Output array for 79 symbols (tones 0-7)
///
/// # Example
/// ```
/// use bitvec::prelude::*;
/// use rustyft8::symbol;
///
/// let mut codeword_storage = [0u8; 22];
/// let codeword = &codeword_storage.view_bits::<Msb0>()[..174];
/// // ... fill codeword with LDPC encoded data ...
///
/// let mut symbols = [0u8; 79];
/// symbol::map(codeword, &mut symbols)?;
/// # Ok::<(), String>(())
/// ```
pub fn map(codeword: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>, symbols: &mut [u8; NN]) -> Result<(), String> {
    if codeword.len() != 174 {
        return Err(format!("Codeword must be exactly 174 bits, got {}", codeword.len()));
    }

    // Insert sync patterns at positions: 0-6, 36-42, 72-78
    symbols[0..7].copy_from_slice(&COSTAS);
    symbols[36..43].copy_from_slice(&COSTAS);
    symbols[72..79].copy_from_slice(&COSTAS);

    // Map data symbols
    // Structure: positions 7-35 (29 symbols), 43-71 (29 symbols)
    let mut k = 7; // Start after first sync

    for j in 0..ND {
        let i = 3 * j; // Bit position in codeword (0, 3, 6, ...)

        // After 29 data symbols, skip over middle sync (k jumps from 35 to 43)
        if j == 29 {
            k += 7;
        }

        // Extract 3 bits and convert to tone using Gray mapping
        let bit0 = codeword[i] as u8;
        let bit1 = codeword[i + 1] as u8;
        let bit2 = codeword[i + 2] as u8;
        let indx = (bit0 << 2) | (bit1 << 1) | bit2;

        symbols[k] = GRAY_MAP[indx as usize];
        k += 1;
    }

    Ok(())
}

/// Demap 79 FT8 symbols back to a 174-bit codeword
///
/// Extracts the 58 data symbols from the received symbol array, ignoring
/// the sync symbols, and converts them back to 174 bits using inverse Gray coding.
///
/// # Arguments
/// * `symbols` - Array of 79 received symbols (tones 0-7)
/// * `codeword` - Output buffer for 174-bit codeword
///
/// # Example
/// ```
/// use bitvec::prelude::*;
/// use rustyft8::symbol;
///
/// let symbols = [0u8; 79]; // Received symbols
/// let mut codeword_storage = [0u8; 22];
/// let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
/// symbol::demap(&symbols, codeword)?;
/// # Ok::<(), String>(())
/// ```
pub fn demap(symbols: &[u8; NN], codeword: &mut bitvec::slice::BitSlice<u8, bitvec::order::Msb0>) -> Result<(), String> {
    if codeword.len() != 174 {
        return Err(format!("Codeword must be exactly 174 bits, got {}", codeword.len()));
    }

    // Extract data symbols from positions: 7-35 (29 symbols), 43-71 (29 symbols)
    let mut k = 7; // Start after first sync

    for j in 0..ND {
        let i = 3 * j; // Bit position in codeword

        // After 29 data symbols, skip over middle sync
        if j == 29 {
            k += 7;
        }

        // Convert tone back to 3 bits using inverse Gray mapping
        let tone = symbols[k];
        let indx = GRAY_MAP_INV[tone as usize];

        codeword.set(i, (indx & 0b100) != 0);
        codeword.set(i + 1, (indx & 0b010) != 0);
        codeword.set(i + 2, (indx & 0b001) != 0);

        k += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitvec::prelude::*;

    #[test]
    fn test_constants() {
        assert_eq!(ND, 58, "58 data symbols expected");
        assert_eq!(NS, 21, "21 sync symbols expected");
        assert_eq!(NN, 79, "79 total symbols expected");
        assert_eq!(NN, ND + NS);
    }

    #[test]
    fn test_costas_pattern() {
        assert_eq!(COSTAS.len(), 7);
        // Verify all values are valid tones (0-7)
        for &tone in &COSTAS {
            assert!(tone < 8);
        }
    }

    #[test]
    fn test_gray_map_bijection() {
        // Verify Gray map is a bijection (one-to-one mapping)
        let mut seen = [false; 8];
        for &tone in &GRAY_MAP {
            assert!(tone < 8);
            assert!(!seen[tone as usize], "Duplicate tone in Gray map");
            seen[tone as usize] = true;
        }
    }

    #[test]
    fn test_gray_map_inverse() {
        // Verify inverse Gray map correctly inverts the forward map
        for i in 0..8 {
            let tone = GRAY_MAP[i];
            let back = GRAY_MAP_INV[tone as usize];
            assert_eq!(back, i as u8, "Gray map inverse failed for index {}", i);
        }
    }

    #[test]
    fn test_symbol_map_all_zeros() {
        let codeword_storage = [0u8; 22];
        let codeword = &codeword_storage.view_bits::<Msb0>()[..174];

        let mut symbols = [0u8; 79];
        map(codeword, &mut symbols).unwrap();

        // Check sync positions have Costas pattern
        assert_eq!(&symbols[0..7], &COSTAS);
        assert_eq!(&symbols[36..43], &COSTAS);
        assert_eq!(&symbols[72..79], &COSTAS);

        // All data symbols should map to tone 0 (Gray map of 0b000 = 0)
        for i in 7..36 {
            assert_eq!(symbols[i], 0, "Data symbol at {} should be 0", i);
        }
        for i in 43..72 {
            assert_eq!(symbols[i], 0, "Data symbol at {} should be 0", i);
        }
    }

    #[test]
    fn test_symbol_map_first_bit_set() {
        let mut codeword_storage = [0u8; 22];
        let codeword_bits = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
        codeword_bits.set(0, true); // Set first bit to 1

        let mut symbols = [0u8; 79];
        map(codeword_bits, &mut symbols).unwrap();

        // First data symbol (position 7) should be Gray(0b100) = 5
        assert_eq!(symbols[7], 5);

        // All other data symbols should be 0
        for i in 8..36 {
            assert_eq!(symbols[i], 0);
        }
        for i in 43..72 {
            assert_eq!(symbols[i], 0);
        }
    }

    #[test]
    fn test_map_demap_roundtrip() {
        // Create a test codeword with various bit patterns
        let mut codeword_storage = [0u8; 22];
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];

        // Set some bits in a pattern
        for i in (0..174).step_by(7) {
            codeword.set(i, true);
        }

        // Map to symbols
        let mut symbols = [0u8; 79];
        map(codeword, &mut symbols).unwrap();

        // Demap back to bits
        let mut recovered_storage = [0u8; 22];
        let recovered = &mut recovered_storage.view_bits_mut::<Msb0>()[..174];
        demap(&symbols, recovered).unwrap();

        // Should match original
        assert_eq!(recovered.len(), 174);
        for i in 0..174 {
            assert_eq!(
                recovered[i], codeword[i],
                "Bit mismatch at position {}",
                i
            );
        }
    }

    #[test]
    fn test_symbol_positions() {
        // Verify the symbol structure: S7 D29 S7 D29 S7
        let codeword_storage = [0u8; 22];
        let codeword = &codeword_storage.view_bits::<Msb0>()[..174];

        let mut symbols = [0u8; 79];
        map(codeword, &mut symbols).unwrap();

        // Positions 0-6: Sync
        assert_eq!(&symbols[0..7], &COSTAS);
        // Positions 7-35: Data (29 symbols)
        // Positions 36-42: Sync
        assert_eq!(&symbols[36..43], &COSTAS);
        // Positions 43-71: Data (29 symbols)
        // Positions 72-78: Sync
        assert_eq!(&symbols[72..79], &COSTAS);
    }

    #[test]
    fn test_invalid_codeword_length_map() {
        let codeword_storage = [0u8; 10];
        let codeword = &codeword_storage.view_bits::<Msb0>()[..80];
        let mut symbols = [0u8; 79];
        let result = map(codeword, &mut symbols);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Codeword must be exactly 174 bits"));
    }

    #[test]
    fn test_invalid_codeword_length_demap() {
        let symbols = [0u8; 79];
        let mut codeword_storage = [0u8; 10];
        let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..80];
        let result = demap(&symbols, codeword);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Codeword must be exactly 174 bits"));
    }

    #[test]
    fn test_symbol_map_against_wsjt_x() {
        // Test against WSJT-X ft8code output for message "CQ SOTA N0YPR/R DM42"
        //
        // 77-bit message:
        // 00000000010111100101100110000000010100100110110011100110110001100111110010001
        //
        // 14-bit CRC:
        // 00001001100101
        //
        // 83 parity bits:
        // 11100110011001101100100111100011101000010001100111111001100110001110011001011110010
        //
        // Expected 79 symbols (from WSJT-X):
        // 3140652 00067121500611657165217553054 3140652 37542165575260315771541421243 3140652

        // Build the 174-bit codeword
        let msg_str = "00000000010111100101100110000000010100100110110011100110110001100111110010001";
        let crc_str = "00001001100101";
        let parity_str = "11100110011001101100100111100011101000010001100111111001100110001110011001011110010";

        let mut codeword_storage = [0u8; 22];
        let codeword_bits = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];

        // Fill in message bits
        for (i, c) in msg_str.chars().enumerate() {
            codeword_bits.set(i, c == '1');
        }
        // Fill in CRC bits
        for (i, c) in crc_str.chars().enumerate() {
            codeword_bits.set(77 + i, c == '1');
        }
        // Fill in parity bits
        for (i, c) in parity_str.chars().enumerate() {
            codeword_bits.set(91 + i, c == '1');
        }

        // Map to symbols
        let mut symbols = [0u8; 79];
        map(codeword_bits, &mut symbols).unwrap();

        // Expected symbol string from WSJT-X (spaces removed)
        let expected_str = "3140652000671215006116571652175530543140652375421655752603157715414212433140652";
        let expected_symbols: Vec<u8> = expected_str
            .chars()
            .map(|c| c.to_digit(10).unwrap() as u8)
            .collect();

        // Verify all 79 symbols match
        for i in 0..79 {
            assert_eq!(
                symbols[i], expected_symbols[i],
                "Symbol mismatch at position {}: got {}, expected {}",
                i, symbols[i], expected_symbols[i]
            );
        }
    }
}
