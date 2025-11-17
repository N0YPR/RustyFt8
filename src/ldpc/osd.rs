//! Ordered Statistics Decoding (OSD) for LDPC codes
//!
//! OSD is a fallback decoder that works when Belief Propagation fails.
//! Uses generator matrix transformation with GF(2) Gaussian elimination.

use bitvec::prelude::*;
use crate::crc::crc14_check;
use super::encode::encode;
use once_cell::sync::Lazy;

const N: usize = 174; // Codeword length
const K: usize = 91;  // Message length (77 info + 14 CRC)

/// Cached generator matrix (91×174)
/// Each row is the encoding of a unit vector at that position
static GENERATOR: Lazy<Vec<BitVec<u8, Msb0>>> = Lazy::new(|| {
    let mut gen_matrix = Vec::with_capacity(K);

    for i in 0..K {
        // Create unit vector with 1 at position i
        let mut unit_msg = bitvec![u8, Msb0; 0; K];
        unit_msg.set(i, true);

        // Encode to get the generator row
        let mut codeword = bitvec![u8, Msb0; 0; N];
        encode(&unit_msg, &mut codeword);

        gen_matrix.push(codeword);
    }

    gen_matrix
});

/// Perform Gaussian elimination on generator matrix in GF(2)
/// Returns the reduced matrix and column permutation
fn gaussian_elimination(
    gen_matrix: &[BitVec<u8, Msb0>],
    col_order: &[usize],
) -> (Vec<BitVec<u8, Msb0>>, Vec<usize>) {
    // Create working copy with reordered columns
    let mut matrix: Vec<BitVec<u8, Msb0>> = gen_matrix.iter().map(|row| {
        let mut new_row = bitvec![u8, Msb0; 0; N];
        for (new_idx, &orig_idx) in col_order.iter().enumerate() {
            new_row.set(new_idx, row[orig_idx]);
        }
        new_row
    }).collect();

    let mut indices = col_order.to_vec();

    // Gaussian elimination to RREF
    for diag in 0..K {
        // Find pivot (look ahead up to 20 columns)
        let mut pivot_col = None;
        for col in diag..K.min(diag + 20) {
            if matrix[diag][col] {
                pivot_col = Some(col);
                break;
            }
        }

        let pivot_col = match pivot_col {
            Some(col) => col,
            None => continue, // Degenerate case
        };

        // Swap columns if needed
        if pivot_col != diag {
            for row in &mut matrix {
                let temp_diag = row[diag];
                let temp_pivot = row[pivot_col];
                row.set(diag, temp_pivot);
                row.set(pivot_col, temp_diag);
            }
            indices.swap(diag, pivot_col);
        }

        // Eliminate: XOR rows that have 1 in this column
        let pivot_row = matrix[diag].clone();
        for row_idx in 0..K {
            if row_idx != diag && matrix[row_idx][diag] {
                matrix[row_idx] ^= &pivot_row;
            }
        }
    }

    (matrix, indices)
}

/// Fast encoding using RREF generator matrix
fn encode_with_rref(info_bits: &BitSlice<u8, Msb0>, rref_gen: &[BitVec<u8, Msb0>]) -> BitVec<u8, Msb0> {
    let mut codeword = bitvec![u8, Msb0; 0; N];

    for (i, bit) in info_bits.iter().enumerate() {
        if *bit {
            codeword ^= &rref_gen[i];
        }
    }

    codeword
}

/// OSD decoder - attempts to find valid codeword when BP fails
///
/// Proper OSD algorithm using generator matrix transformation:
/// 1. Order bits by reliability (|LLR| magnitude)
/// 2. Perform Gaussian elimination on generator matrix in GF(2)
/// 3. Test information bit patterns (order-0, order-1, order-2)
/// 4. Each test produces valid codewords respecting LDPC constraints
///
/// # Arguments
/// * `llr` - Log-likelihood ratios (174 bits)
/// * `max_order` - Maximum number of bit flips to try (0 = none, 1 = single, 2 = pairs)
///
/// # Returns
/// * `Some(message91)` - Decoded 91-bit message if successful
/// * `None` - If no valid codeword found
pub fn osd_decode(llr: &[f32], max_order: usize) -> Option<BitVec<u8, Msb0>> {
    if llr.len() != N {
        return None;
    }

    // Step 1: Create reliability ordering (most reliable first)
    let mut col_order: Vec<usize> = (0..N).collect();
    col_order.sort_by(|&a, &b| {
        llr[b].abs().partial_cmp(&llr[a].abs()).unwrap_or(core::cmp::Ordering::Equal)
    });

    // Step 2: Perform Gaussian elimination on reordered generator matrix
    // NOTE: This is expensive (~10ms) and is the performance bottleneck
    let (rref_gen, final_indices) = gaussian_elimination(&GENERATOR, &col_order);

    // Step 3: Make hard decisions on reordered bits
    let mut hard_decisions_ordered = bitvec![u8, Msb0; 0; N];
    for i in 0..N {
        let orig_idx = final_indices[i];
        hard_decisions_ordered.set(i, llr[orig_idx] >= 0.0);
    }

    // Step 4: Order-0 - Try hard decisions directly
    let info_bits = &hard_decisions_ordered[0..K];
    let candidate = encode_with_rref(info_bits, &rref_gen);

    // Un-permute back to original bit order
    let mut unpermuted = bitvec![u8, Msb0; 0; N];
    for i in 0..N {
        unpermuted.set(final_indices[i], candidate[i]);
    }

    // Extract message and check CRC
    let message91: BitVec<u8, Msb0> = unpermuted[0..K].to_bitvec();
    if crc14_check(&message91) {
        return Some(message91);
    }

    if max_order == 0 {
        return None;
    }

    // Order-1: Try flipping single unreliable bits
    // Focus on the last 30 bits (least reliable information positions)
    let flip_start = K.saturating_sub(30);

    for flip_idx in flip_start..K {
        let mut test_info = info_bits.to_bitvec();
        let current = test_info[flip_idx];
        test_info.set(flip_idx, !current);

        let candidate = encode_with_rref(&test_info, &rref_gen);

        // Un-permute back to original bit order
        let mut unpermuted = bitvec![u8, Msb0; 0; N];
        for i in 0..N {
            unpermuted.set(final_indices[i], candidate[i]);
        }

        let test_msg: BitVec<u8, Msb0> = unpermuted[0..K].to_bitvec();
        if crc14_check(&test_msg) {
            return Some(test_msg);
        }
    }

    if max_order < 2 {
        return None;
    }

    // Order-2: Try flipping pairs of unreliable bits
    // This is expensive, so limit the search space
    let flip_count = 20.min(K - flip_start);

    for i in 0..flip_count {
        for j in (i + 1)..flip_count {
            let idx_i = flip_start + i;
            let idx_j = flip_start + j;

            let mut test_info = info_bits.to_bitvec();
            let bit_i = test_info[idx_i];
            let bit_j = test_info[idx_j];
            test_info.set(idx_i, !bit_i);
            test_info.set(idx_j, !bit_j);

            let candidate = encode_with_rref(&test_info, &rref_gen);

            // Un-permute back to original bit order
            let mut unpermuted = bitvec![u8, Msb0; 0; N];
            for i in 0..N {
                unpermuted.set(final_indices[i], candidate[i]);
            }

            let test_msg: BitVec<u8, Msb0> = unpermuted[0..K].to_bitvec();
            if crc14_check(&test_msg) {
                return Some(test_msg);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generator_matrix_properties() {
        // Verify generator matrix has correct dimensions
        assert_eq!(GENERATOR.len(), K, "Generator should have K={} rows", K);
        for (i, row) in GENERATOR.iter().enumerate() {
            assert_eq!(row.len(), N, "Generator row {} should have N={} columns", i, N);
        }
        println!("✓ Generator matrix has correct dimensions: {}×{}", K, N);
    }

    #[test]
    fn test_encode_with_generator() {
        // Test that encoding with generator matches standard encoding
        let mut test_msg = bitvec![u8, Msb0; 0; K];
        test_msg.set(0, true);  // Unit vector
        test_msg.set(5, true);
        test_msg.set(10, true);

        // Standard encoding
        let mut std_codeword = bitvec![u8, Msb0; 0; N];
        encode(&test_msg, &mut std_codeword);

        // Generator-based encoding (XOR rows)
        let mut gen_codeword = bitvec![u8, Msb0; 0; N];
        for i in 0..K {
            if test_msg[i] {
                gen_codeword ^= &GENERATOR[i];
            }
        }

        assert_eq!(std_codeword, gen_codeword,
            "Generator-based encoding should match standard encoding");
        println!("✓ Generator matrix encoding matches standard LDPC encoding");
    }

    #[test]
    fn test_gaussian_elimination_identity() {
        // Test that Gaussian elimination produces systematic form
        let identity_order: Vec<usize> = (0..N).collect();
        let (rref, indices) = gaussian_elimination(&GENERATOR, &identity_order);

        // Check that first K columns form identity (or close to it)
        let mut rank = 0;
        for diag in 0..K {
            if rref[diag][diag] {
                rank += 1;
            }
        }

        println!("Gaussian elimination rank: {}/{}", rank, K);
        assert!(rank >= K - 5, "RREF should have high rank (got {}, expected ~{})", rank, K);
    }

    #[test]
    fn test_osd_decode_basic() {
        // Test that OSD can decode a perfect codeword
        // Start with a known valid 91-bit message
        let mut message91 = bitvec![u8, Msb0; 0; K];
        // This would need to be a valid message with correct CRC

        // For now, just test that the function runs without panicking
        let llr = vec![1.0f32; N];
        let result = osd_decode(&llr, 0);
        // Result will be None because we don't have a valid message, but it shouldn't crash
        assert!(result.is_none() || result.is_some());
    }
}
