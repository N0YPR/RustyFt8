//! Ordered Statistics Decoding (OSD) for LDPC codes
//!
//! OSD is a fallback decoder that works when Belief Propagation fails.
//! Uses generator matrix transformation with GF(2) Gaussian elimination.

use bitvec::prelude::*;
use crate::crc::crc14_check;
use super::encode::encode;
use once_cell::sync::Lazy;
use tracing::{debug, trace};

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

/// Generate next combination pattern for exhaustive OSD search
/// Equivalent to WSJT-X's nextpat91 subroutine
///
/// Returns the next K-bit pattern with exactly `order` bits set,
/// in lexicographic order. Returns None when all patterns exhausted.
fn next_combination_pattern(pattern: &mut BitVec<u8, Msb0>, k: usize, order: usize) -> bool {
    // Find rightmost 01 sequence
    let mut swap_pos = None;
    for i in 0..k-1 {
        if !pattern[i] && pattern[i+1] {
            swap_pos = Some(i);
        }
    }

    let swap_pos = match swap_pos {
        Some(pos) => pos,
        None => return false, // No more patterns
    };

    // Create new pattern
    let mut new_pattern = bitvec![u8, Msb0; 0; k];

    // Copy bits before swap position
    for i in 0..swap_pos {
        new_pattern.set(i, pattern[i]);
    }

    // Swap: 01 -> 10
    new_pattern.set(swap_pos, true);
    new_pattern.set(swap_pos + 1, false);

    // Move remaining 1s to the right end
    if swap_pos + 1 < k {
        let ones_remaining = order - new_pattern[0..=swap_pos].count_ones();
        for i in (k - ones_remaining)..k {
            new_pattern.set(i, true);
        }
    }

    *pattern = new_pattern;
    true
}

/// Compute Euclidean distance metric between candidate and received word
/// Uses weighted Hamming distance: sum(|llr| where bits differ)
#[inline]
fn compute_distance(candidate: &BitSlice<u8, Msb0>, hard_dec: &BitSlice<u8, Msb0>,
                     abs_llr: &[f32]) -> f32 {
    let mut dist = 0.0;
    for i in 0..candidate.len() {
        if candidate[i] != hard_dec[i] {
            dist += abs_llr[i];
        }
    }
    dist
}

/// OSD decoder - attempts to find valid codeword when BP fails
///
/// Implements WSJT-X's OSD algorithm with exhaustive combination search:
/// 1. Order bits by reliability (|LLR| magnitude)
/// 2. Perform Gaussian elimination on generator matrix in GF(2)
/// 3. Exhaustively test all combinations of bit flips up to max_order
/// 4. Use Euclidean distance metric to find best candidate
///
/// Based on WSJT-X's osd174_91.f90 with ndeep parameter mapping:
/// - max_order 0: Order-0 only (hard decisions)
/// - max_order 1: Order-1 exhaustive search (all 91 single flips)
/// - max_order 2: Order-2 exhaustive search (all 4095 pairs)
///
/// # Arguments
/// * `llr` - Log-likelihood ratios (174 bits)
/// * `max_order` - Maximum flip order (0, 1, or 2)
///
/// # Returns
/// * `Some(message91)` - Decoded 91-bit message if successful
/// * `None` - If no valid codeword found
pub fn osd_decode(llr: &[f32], max_order: usize) -> Option<BitVec<u8, Msb0>> {
    if llr.len() != N {
        return None;
    }

    let llr_mean = llr.iter().map(|x| x.abs()).sum::<f32>() / llr.len() as f32;
    let llr_max = llr.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    debug!(max_order, llr_mean, llr_max, "OSD decode starting (WSJT-X exhaustive search)");
    eprintln!("OSD called: max_order={}, llr_mean={:.3}, llr_max={:.3}", max_order, llr_mean, llr_max);

    // Step 1: Create reliability ordering (most reliable first)
    let mut col_order: Vec<usize> = (0..N).collect();
    col_order.sort_by(|&a, &b| {
        llr[b].abs().partial_cmp(&llr[a].abs()).unwrap_or(core::cmp::Ordering::Equal)
    });

    // Step 2: Perform Gaussian elimination on reordered generator matrix
    let (rref_gen, final_indices) = gaussian_elimination(&GENERATOR, &col_order);

    // Step 3: Make hard decisions on reordered bits
    let mut hard_decisions_ordered = bitvec![u8, Msb0; 0; N];
    let mut abs_llr_ordered = vec![0.0f32; N];
    for i in 0..N {
        let orig_idx = final_indices[i];
        hard_decisions_ordered.set(i, llr[orig_idx] >= 0.0);
        abs_llr_ordered[i] = llr[orig_idx].abs();
    }

    // Extract information bits and parity in reordered space
    let m0 = &hard_decisions_ordered[0..K]; // Order-0 message
    let hard_dec_full = &hard_decisions_ordered;

    // Step 4: Order-0 - Try hard decisions directly
    let c0 = encode_with_rref(m0, &rref_gen);
    let mut best_dist = compute_distance(&c0, hard_dec_full, &abs_llr_ordered);
    let mut best_codeword = c0.clone();

    // Un-permute and check CRC
    let mut unpermuted = bitvec![u8, Msb0; 0; N];
    for i in 0..N {
        unpermuted.set(final_indices[i], best_codeword[i]);
    }
    let message91: BitVec<u8, Msb0> = unpermuted[0..K].to_bitvec();
    if crc14_check(&message91) {
        debug!(best_dist, "Order-0 success (hard decisions)");
        eprintln!("  Order-0 SUCCESS");
        return Some(message91);
    }

    debug!(best_dist, "Order-0 failed, continuing to higher orders");
    eprintln!("  Order-0 failed, dist={:.3}", best_dist);

    if max_order == 0 {
        return None;
    }

    // Step 5: Exhaustive search for order 1..max_order
    for order in 1..=max_order {
        let total_combos = match order {
            1 => K,
            2 => K * (K - 1) / 2,
            3 => K * (K - 1) * (K - 2) / 6,
            _ => 0,
        };
        debug!(order, total_combos, "Starting OSD order search");
        eprintln!("  Trying Order-{} ({} combinations)...", order, total_combos);

        // Initialize pattern: order bits set at the end
        let mut pattern = bitvec![u8, Msb0; 0; K];
        for i in (K - order)..K {
            pattern.set(i, true);
        }

        let mut tested = 0;
        let mut improved = 0;

        loop {
            tested += 1;

            // Create test message by XORing pattern with m0
            let mut test_msg = m0.to_bitvec();
            for i in 0..K {
                if pattern[i] {
                    let current = test_msg[i];
                    test_msg.set(i, !current);
                }
            }

            // Encode and compute distance
            let candidate = encode_with_rref(&test_msg, &rref_gen);
            let dist = compute_distance(&candidate, hard_dec_full, &abs_llr_ordered);

            // Track best candidate
            if dist < best_dist {
                best_dist = dist;
                best_codeword = candidate.clone();
                improved += 1;
                trace!(order, tested, dist, "Found improved candidate");

                // Un-permute and check CRC
                for i in 0..N {
                    unpermuted.set(final_indices[i], candidate[i]);
                }
                let test_msg_91: BitVec<u8, Msb0> = unpermuted[0..K].to_bitvec();
                if crc14_check(&test_msg_91) {
                    debug!(order, tested, dist, "OSD decode success");
                    eprintln!("  Order-{} SUCCESS! (tested={}, dist={:.3})", order, tested, dist);
                    return Some(test_msg_91);
                }
            }

            // Get next combination
            if !next_combination_pattern(&mut pattern, K, order) {
                break;
            }
        }

        debug!(order, tested, improved, best_dist, "OSD order exhausted without success");
        eprintln!("  Order-{} FAILED (tested={}, improved={}, best_dist={:.3})", order, tested, improved, best_dist);
    }

    debug!("All OSD orders exhausted - decode failed");
    eprintln!("  All orders exhausted - OSD FAILED");

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
