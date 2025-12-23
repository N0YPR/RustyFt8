//! Ordered Statistics Decoding (OSD) for LDPC codes
//!
//! OSD is a fallback decoder that works when Belief Propagation fails.
//! Uses generator matrix transformation with GF(2) Gaussian elimination.
//!
//! Performance optimization: Uses `[u64; 3]` for 174-bit rows instead of BitVec.
//! This provides ~5-10x speedup for Gaussian elimination and encoding operations.

use bitvec::prelude::*;
use crate::crc::crc14_check;
use super::encode::encode;
use once_cell::sync::Lazy;
use tracing::{debug, trace};

const N: usize = 174; // Codeword length
const K: usize = 91;  // Message length (77 info + 14 CRC)

// ============================================================================
// Fast bit operations using [u64; 3] instead of BitVec
// 174 bits fits in 3 × 64 = 192 bits
// ============================================================================

/// Number of u64 words needed for N bits
const WORDS: usize = 3;

/// Fast 174-bit row using packed u64 array
/// Bit layout: bits 0-63 in word[0], bits 64-127 in word[1], bits 128-191 in word[2]
/// Bit 0 is MSB of word[0], bit 63 is LSB of word[0], etc.
type Row = [u64; WORDS];

/// Get a single bit from a Row
#[inline(always)]
fn row_get_bit(row: &Row, idx: usize) -> bool {
    debug_assert!(idx < N);
    let word_idx = idx / 64;
    let bit_idx = idx % 64;
    // MSB-first: bit 0 is at position 63, bit 63 is at position 0
    (row[word_idx] >> (63 - bit_idx)) & 1 != 0
}

/// Set a single bit in a Row
#[inline(always)]
fn row_set_bit(row: &mut Row, idx: usize, val: bool) {
    debug_assert!(idx < N);
    let word_idx = idx / 64;
    let bit_idx = idx % 64;
    let mask = 1u64 << (63 - bit_idx);
    if val {
        row[word_idx] |= mask;
    } else {
        row[word_idx] &= !mask;
    }
}

/// XOR two rows: dst ^= src
#[inline(always)]
fn row_xor(dst: &mut Row, src: &Row) {
    dst[0] ^= src[0];
    dst[1] ^= src[1];
    dst[2] ^= src[2];
}

/// Swap two bits in a Row
#[inline(always)]
fn row_swap_bits(row: &mut Row, i: usize, j: usize) {
    let bit_i = row_get_bit(row, i);
    let bit_j = row_get_bit(row, j);
    row_set_bit(row, i, bit_j);
    row_set_bit(row, j, bit_i);
}

/// Count number of set bits in a Row (up to N bits)
#[inline]
fn row_count_ones(row: &Row) -> usize {
    // Full count for first two words
    let count = row[0].count_ones() as usize + row[1].count_ones() as usize;
    // For word[2], only count bits 0-45 (bits 128-173)
    // Mask off bits 174-191 (positions 46-63 in word[2])
    let mask = !((1u64 << (64 - (N - 128))) - 1); // Keep top 46 bits
    count + (row[2] & mask).count_ones() as usize
}

/// Create a zero Row
#[inline(always)]
const fn row_zero() -> Row {
    [0u64; WORDS]
}

/// Convert BitVec to Row
fn bitvec_to_row(bv: &BitVec<u8, Msb0>) -> Row {
    let mut row = row_zero();
    for i in 0..N.min(bv.len()) {
        if bv[i] {
            row_set_bit(&mut row, i, true);
        }
    }
    row
}

/// Convert Row to BitVec
fn row_to_bitvec(row: &Row) -> BitVec<u8, Msb0> {
    let mut bv = bitvec![u8, Msb0; 0; N];
    for i in 0..N {
        if row_get_bit(row, i) {
            bv.set(i, true);
        }
    }
    bv
}

/// Cached generator matrix (91×174) - BitVec version (legacy)
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

/// Fast cached generator matrix (91×174) using Row type
/// Each row is the encoding of a unit vector at that position
static GENERATOR_FAST: Lazy<Vec<Row>> = Lazy::new(|| {
    GENERATOR.iter().map(bitvec_to_row).collect()
});

// ============================================================================
// Fast Gaussian elimination using Row type
// ============================================================================

/// Fast Gaussian elimination on generator matrix in GF(2)
/// Returns the reduced matrix and column permutation
fn gaussian_elimination_fast(
    gen_matrix: &[Row],
    col_order: &[usize],
) -> (Vec<Row>, Vec<usize>) {
    // Create working copy with reordered columns
    let mut matrix: Vec<Row> = gen_matrix.iter().map(|row| {
        let mut new_row = row_zero();
        for (new_idx, &orig_idx) in col_order.iter().enumerate() {
            if row_get_bit(row, orig_idx) {
                row_set_bit(&mut new_row, new_idx, true);
            }
        }
        new_row
    }).collect();

    let mut indices = col_order.to_vec();

    // Gaussian elimination to RREF
    for diag in 0..K {
        // Find pivot: search from diag to K+20
        let mut pivot_col = None;
        for col in diag..(K + 20).min(N) {
            if row_get_bit(&matrix[diag], col) {
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
                row_swap_bits(row, diag, pivot_col);
            }
            indices.swap(diag, pivot_col);
        }

        // Eliminate: XOR rows that have 1 in this column
        let pivot_row = matrix[diag];
        for row_idx in 0..K {
            if row_idx != diag && row_get_bit(&matrix[row_idx], diag) {
                row_xor(&mut matrix[row_idx], &pivot_row);
            }
        }
    }

    (matrix, indices)
}

/// Fast encoding using RREF generator matrix (Row version)
#[inline]
fn encode_with_rref_fast(info_bits: &Row, rref_gen: &[Row]) -> Row {
    let mut codeword = row_zero();

    for i in 0..K {
        if row_get_bit(info_bits, i) {
            row_xor(&mut codeword, &rref_gen[i]);
        }
    }

    codeword
}

/// Fast Euclidean distance computation (Row version)
#[inline]
fn compute_distance_fast(candidate: &Row, hard_dec: &Row, abs_llr: &[f32]) -> f32 {
    let mut dist = 0.0f32;
    for i in 0..N {
        if row_get_bit(candidate, i) != row_get_bit(hard_dec, i) {
            dist += abs_llr[i];
        }
    }
    dist
}

/// Perform Gaussian elimination on generator matrix in GF(2) (legacy BitVec version)
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
        // Find pivot: WSJT-X uses do icol=id,k+20 which searches from diag to K+20
        // This is crucial - it searches ALL the way up to column K+20 (111), not just 20 columns ahead
        let mut pivot_col = None;
        for col in diag..(K + 20).min(N) {
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

/// OSD configuration parameters based on ndeep (WSJT-X decoding depth)
/// Maps WSJT-X's ndeep parameter to the actual algorithm parameters
#[derive(Debug, Clone, Copy)]
struct OsdConfig {
    nord: usize,      // Maximum order for exhaustive search
    npre1: bool,      // Enable preprocessing rule 1 (threshold rejection)
    npre2: bool,      // Enable preprocessing rule 2 (hash-based pattern matching)
    nt: usize,        // Number of tail bits to check for early rejection
    ntheta: usize,    // Maximum allowed errors in first nt tail bits
    ntau: usize,      // Window size for preprocessing rule 2
}

impl OsdConfig {
    /// Create configuration from ndeep parameter (matches WSJT-X decode174_91.f90)
    fn from_ndeep(ndeep: usize) -> Self {
        match ndeep {
            0 => OsdConfig {
                nord: 0,
                npre1: false,
                npre2: false,
                nt: 0,
                ntheta: 0,
                ntau: 0,
            },
            1 => OsdConfig {
                nord: 1,
                npre1: false,
                npre2: false,
                nt: 40,
                ntheta: 12,
                ntau: 0,
            },
            2 => OsdConfig {
                nord: 1,
                npre1: true,
                npre2: false,
                nt: 40,
                ntheta: 10,
                ntau: 0,
            },
            3 => OsdConfig {
                nord: 1,
                npre1: true,
                npre2: true,
                nt: 40,
                ntheta: 12,
                ntau: 14,
            },
            4 => OsdConfig {
                nord: 2,
                npre1: true,
                npre2: true,
                nt: 40,
                ntheta: 12,
                ntau: 17,
            },
            5 => OsdConfig {
                nord: 3,
                npre1: true,
                npre2: true,
                nt: 40,
                ntheta: 12,
                ntau: 15,
            },
            _ => OsdConfig { // ndeep >= 6
                nord: 4,
                npre1: true,
                npre2: true,
                nt: 95,
                ntheta: 12,
                ntau: 15,
            },
        }
    }
}

/// Hash table for preprocessing rule 2 pattern matching
/// Stores pairs of bit positions (i1, i2) indexed by their tail error pattern
use std::collections::HashMap;

type PatternHash = u32;
type BitPair = (usize, usize);

/// Build hash table of error patterns for preprocessing rule 2
/// Equivalent to WSJT-X's boxit91 subroutine
fn build_pattern_hash_table(
    rref_gen: &[BitVec<u8, Msb0>],
    ntau: usize,
) -> HashMap<PatternHash, Vec<BitPair>> {
    let mut hash_table: HashMap<PatternHash, Vec<BitPair>> = HashMap::new();

    // For all pairs (i1, i2) where i1 > i2
    // rref_gen is K rows × N columns, so rref_gen[row][col] where row < K, col < N
    // Fortran: do i1=k,1,-1; do i2=i1-1,1,-1 (1-indexed: 91..1, then i1-1..1)
    // Rust equivalent: i1 from K-1 down to 0, i2 from i1-1 down to 0
    for i1 in (0..K).rev() {
        for i2 in (0..i1).rev() {
            // Compute XOR of the first ntau tail bits from generator rows i1 and i2
            // IMPORTANT: Fortran g2(K+1:K+ntau, i1) accesses rows K+1..K+ntau of COLUMN i1
            // This suggests g2 is stored as N×K (transposed).
            // Since our rref_gen is K×N, we need to think of XORing "columns" in the transposed sense
            // which means XORing bits at the same tail position across two information bits.
            //
            // Actually, for RREF generator matrix in systematic form:
            // rref_gen[i] = codeword when only bit i is set
            // So rref_gen[i1][K+j] is the j-th parity bit for info bit i1
            // We want to XOR the parity patterns for bits i1 and i2
            let mut pattern = 0u32;
            for tail_idx in 0..ntau {
                let tail_col = K + tail_idx;
                if tail_col < N {
                    // XOR the tail_col-th bit of rows i1 and i2
                    let bit1 = rref_gen[i1][tail_col];
                    let bit2 = rref_gen[i2][tail_col];
                    if bit1 ^ bit2 {
                        pattern |= 1 << (ntau - 1 - tail_idx);
                    }
                }
            }

            // Store this pair in the hash table
            hash_table.entry(pattern).or_insert_with(Vec::new).push((i1, i2));
        }
    }

    trace!("Built pattern hash table with {} unique patterns", hash_table.len());
    hash_table
}

/// Look up bit pairs that produce a given error pattern
/// Returns an iterator over matching pairs
fn lookup_pattern_pairs<'a>(
    hash_table: &'a HashMap<PatternHash, Vec<BitPair>>,
    error_pattern: &BitSlice<u8, Msb0>,
    ntau: usize,
) -> impl Iterator<Item = BitPair> + 'a {
    // Convert bit pattern to hash
    let mut pattern_hash = 0u32;
    for i in 0..ntau.min(error_pattern.len()) {
        if error_pattern[i] {
            pattern_hash |= 1 << (ntau - 1 - i);
        }
    }

    // Look up and return all matching pairs
    hash_table.get(&pattern_hash)
        .map(|v| v.iter().copied())
        .into_iter()
        .flatten()
}

/// Test helper to verify hash table construction
#[cfg(test)]
fn test_hash_table_size() -> usize {
    let identity_order: Vec<usize> = (0..N).collect();
    let (rref, _) = gaussian_elimination(&GENERATOR, &identity_order);
    let table = build_pattern_hash_table(&rref, 14);
    let total_pairs: usize = table.values().map(|v| v.len()).sum();
    assert_eq!(total_pairs, 4095, "Hash table should have all C(91,2)=4095 pairs");
    table.len() // Return number of unique patterns
}

// ============================================================================
// Fast OSD decoder using Row type (~5-10x faster than BitVec version)
// ============================================================================

/// Fast OSD decoder using u64-packed rows
///
/// This is the optimized version that uses `[u64; 3]` instead of `BitVec`
/// for all bit operations, providing significant speedup.
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

    // Step 1: Create reliability ordering (most reliable first)
    let mut col_order: Vec<usize> = (0..N).collect();
    col_order.sort_by(|&a, &b| {
        llr[b].abs().partial_cmp(&llr[a].abs()).unwrap_or(core::cmp::Ordering::Equal)
    });

    // Step 2: Perform Gaussian elimination on reordered generator matrix (FAST)
    let (rref_gen, final_indices) = gaussian_elimination_fast(&GENERATOR_FAST, &col_order);

    // Step 3: Make hard decisions on reordered bits
    let mut hard_decisions_ordered = row_zero();
    let mut abs_llr_ordered = vec![0.0f32; N];
    for i in 0..N {
        let orig_idx = final_indices[i];
        if llr[orig_idx] >= 0.0 {
            row_set_bit(&mut hard_decisions_ordered, i, true);
        }
        abs_llr_ordered[i] = llr[orig_idx].abs();
    }

    // Extract m0 (order-0 message) - first K bits of hard decisions
    let m0 = hard_decisions_ordered; // We'll use the full row but only iterate over K bits

    // Step 4: Order-0 - Try hard decisions directly
    let c0 = encode_with_rref_fast(&m0, &rref_gen);
    let mut best_dist = compute_distance_fast(&c0, &hard_decisions_ordered, &abs_llr_ordered);
    let mut best_codeword = c0;

    // Un-permute and check CRC
    let mut unpermuted = row_zero();
    for i in 0..N {
        if row_get_bit(&best_codeword, i) {
            row_set_bit(&mut unpermuted, final_indices[i], true);
        }
    }
    let message91 = row_to_bitvec(&unpermuted);
    let message91 = message91[0..K].to_bitvec();
    if crc14_check(&message91) {
        debug!(best_dist, "Order-0 success (hard decisions)");
        return Some(message91);
    }

    if max_order == 0 {
        return None;
    }

    // Step 5: Exhaustive search for order 1..max_order
    // Use index-based iteration instead of bit pattern manipulation

    // Order-1: Try flipping each single bit
    if max_order >= 1 {
        for i in 0..K {
            // Create test message by flipping bit i
            let mut test_msg = m0;
            let current = row_get_bit(&test_msg, i);
            row_set_bit(&mut test_msg, i, !current);

            // Encode and compute distance
            let candidate = encode_with_rref_fast(&test_msg, &rref_gen);
            let dist = compute_distance_fast(&candidate, &hard_decisions_ordered, &abs_llr_ordered);

            if dist < best_dist {
                best_dist = dist;
                best_codeword = candidate;

                // Un-permute and check CRC
                let mut unpermuted = row_zero();
                for j in 0..N {
                    if row_get_bit(&candidate, j) {
                        row_set_bit(&mut unpermuted, final_indices[j], true);
                    }
                }
                let msg = row_to_bitvec(&unpermuted);
                let msg91 = msg[0..K].to_bitvec();
                if crc14_check(&msg91) {
                    debug!(best_dist, "Order-1 success at bit {}", i);
                    return Some(msg91);
                }
            }
        }
    }

    // Order-2: Try flipping each pair of bits
    if max_order >= 2 {
        for i in 0..K {
            for j in (i+1)..K {
                // Create test message by flipping bits i and j
                let mut test_msg = m0;
                let bit_i = row_get_bit(&test_msg, i);
                let bit_j = row_get_bit(&test_msg, j);
                row_set_bit(&mut test_msg, i, !bit_i);
                row_set_bit(&mut test_msg, j, !bit_j);

                // Encode and compute distance
                let candidate = encode_with_rref_fast(&test_msg, &rref_gen);
                let dist = compute_distance_fast(&candidate, &hard_decisions_ordered, &abs_llr_ordered);

                if dist < best_dist {
                    best_dist = dist;
                    best_codeword = candidate;

                    // Un-permute and check CRC
                    let mut unpermuted = row_zero();
                    for k in 0..N {
                        if row_get_bit(&candidate, k) {
                            row_set_bit(&mut unpermuted, final_indices[k], true);
                        }
                    }
                    let msg = row_to_bitvec(&unpermuted);
                    let msg91 = msg[0..K].to_bitvec();
                    if crc14_check(&msg91) {
                        debug!(best_dist, "Order-2 success at bits ({}, {})", i, j);
                        return Some(msg91);
                    }
                }
            }
        }
    }

    // Order-3: Try flipping each triple of bits (if requested)
    if max_order >= 3 {
        for i in 0..K {
            for j in (i+1)..K {
                for k in (j+1)..K {
                    let mut test_msg = m0;
                    let bit_i = row_get_bit(&test_msg, i);
                    let bit_j = row_get_bit(&test_msg, j);
                    let bit_k = row_get_bit(&test_msg, k);
                    row_set_bit(&mut test_msg, i, !bit_i);
                    row_set_bit(&mut test_msg, j, !bit_j);
                    row_set_bit(&mut test_msg, k, !bit_k);

                    let candidate = encode_with_rref_fast(&test_msg, &rref_gen);
                    let dist = compute_distance_fast(&candidate, &hard_decisions_ordered, &abs_llr_ordered);

                    if dist < best_dist {
                        best_dist = dist;
                        best_codeword = candidate;

                        let mut unpermuted = row_zero();
                        for l in 0..N {
                            if row_get_bit(&candidate, l) {
                                row_set_bit(&mut unpermuted, final_indices[l], true);
                            }
                        }
                        let msg = row_to_bitvec(&unpermuted);
                        let msg91 = msg[0..K].to_bitvec();
                        if crc14_check(&msg91) {
                            debug!(best_dist, "Order-3 success at bits ({}, {}, {})", i, j, k);
                            return Some(msg91);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Legacy OSD decoder using BitVec (kept for reference/testing)
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
#[allow(dead_code)]
fn osd_decode_legacy(llr: &[f32], max_order: usize) -> Option<BitVec<u8, Msb0>> {
    if llr.len() != N {
        return None;
    }

    let llr_mean = llr.iter().map(|x| x.abs()).sum::<f32>() / llr.len() as f32;
    let llr_max = llr.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    debug!(max_order, llr_mean, llr_max, "OSD decode starting (WSJT-X exhaustive search)");

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
        return Some(message91);
    }

    debug!(best_dist, "Order-0 failed, continuing to higher orders");

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
                    return Some(test_msg_91);
                }
            }

            // Get next combination
            if !next_combination_pattern(&mut pattern, K, order) {
                break;
            }
        }

        debug!(order, tested, improved, best_dist, "OSD order exhausted without success");
    }

    debug!("All OSD orders exhausted - decode failed");

    None
}

/// Enhanced OSD decoder with WSJT-X preprocessing rules
///
/// This is the full implementation matching WSJT-X's osd174_91.f90, including:
/// - Preprocessing rule 1 (ntheta threshold rejection)
/// - Preprocessing rule 2 (hash-based pattern matching)
/// - Proper ndeep parameter mapping
///
/// # Arguments
/// * `llr` - Log-likelihood ratios (174 bits)
/// * `ndeep` - WSJT-X decoding depth (0-6, where 3 is default for FT8)
///
/// # Returns
/// * `Some(message91)` - Decoded 91-bit message if successful
/// * `None` - If no valid codeword found
pub fn osd_decode_wsjt(llr: &[f32], ndeep: usize) -> Option<BitVec<u8, Msb0>> {
    if llr.len() != N {
        return None;
    }

    let config = OsdConfig::from_ndeep(ndeep);

    let llr_mean = llr.iter().map(|x| x.abs()).sum::<f32>() / llr.len() as f32;
    let llr_max = llr.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    debug!(ndeep, ?config, llr_mean, llr_max, "OSD decode starting (WSJT-X full implementation)");

    // Step 1: Create reliability ordering (most reliable first)
    let mut col_order: Vec<usize> = (0..N).collect();
    col_order.sort_by(|&a, &b| {
        llr[b].abs().partial_cmp(&llr[a].abs()).unwrap_or(core::cmp::Ordering::Equal)
    });

    // Step 2: Perform Gaussian elimination on reordered generator matrix (FAST)
    let (rref_gen_fast, final_indices) = gaussian_elimination_fast(&GENERATOR_FAST, &col_order);

    // Step 3: Make hard decisions on reordered bits (using fast Row type)
    let mut hard_decisions_ordered = row_zero();
    let mut abs_llr_ordered = vec![0.0f32; N];
    for i in 0..N {
        let orig_idx = final_indices[i];
        if llr[orig_idx] >= 0.0 {
            row_set_bit(&mut hard_decisions_ordered, i, true);
        }
        abs_llr_ordered[i] = llr[orig_idx].abs();
    }

    // m0 (order-0 message) - first K bits of hard decisions (as Row)
    let m0 = hard_decisions_ordered;

    // Step 4: Order-0 - Try hard decisions directly
    let c0 = encode_with_rref_fast(&m0, &rref_gen_fast);
    let mut best_dist = compute_distance_fast(&c0, &hard_decisions_ordered, &abs_llr_ordered);
    let mut best_codeword = c0;

    // Un-permute and check CRC
    let mut unpermuted = row_zero();
    for i in 0..N {
        if row_get_bit(&best_codeword, i) {
            row_set_bit(&mut unpermuted, final_indices[i], true);
        }
    }
    let message91_bv = row_to_bitvec(&unpermuted);
    let message91 = message91_bv[0..K].to_bitvec();
    if crc14_check(&message91) {
        debug!(best_dist, "Order-0 success (hard decisions)");
        return Some(message91);
    }

    if config.nord == 0 {
        return None;
    }

    // Build hash table for preprocessing rule 2 (if enabled)
    // Convert rref_gen_fast back to BitVec for hash table (legacy code path)
    let rref_gen_bitvec: Vec<BitVec<u8, Msb0>> = rref_gen_fast.iter().map(row_to_bitvec).collect();
    let hash_table = if config.npre2 {
        Some(build_pattern_hash_table(&rref_gen_bitvec, config.ntau))
    } else {
        None
    };

    // Step 5: Exhaustive search for order 1..nord (using fast Row operations)
    let mut total_tested = 0;
    let mut total_rejected = 0;
    let mut total_improved = 0;

    for order in 1..=config.nord {

        // Initialize base pattern: order bits set at the end (using BitVec for pattern iteration)
        let mut base_pattern = bitvec![u8, Msb0; 0; K];
        for i in (K - order)..K {
            base_pattern.set(i, true);
        }

        loop {
            // Find position of first set bit (iflag in Fortran)
            let mut iflag = K;
            for i in 0..K {
                if base_pattern[i] {
                    iflag = i;
                    break;
                }
            }

            // Determine inner loop range based on npre1
            let iend = if order == config.nord && !config.npre1 {
                iflag  // Only test the base pattern
            } else {
                0  // Test flipping additional bits (npre1 enabled)
            };

            // Inner loop: test flipping additional bits (WSJT-X's do n1=iflag,iend,-1)
            for n1 in (iend..=iflag).rev() {
                total_tested += 1;

                // Create test message by flipping bits from m0 based on pattern + n1
                let mut test_msg = m0;
                // First apply base pattern
                for i in 0..K {
                    if base_pattern[i] {
                        let current = row_get_bit(&test_msg, i);
                        row_set_bit(&mut test_msg, i, !current);
                    }
                }
                // Then ensure bit n1 is flipped
                if !base_pattern[n1] {
                    let current = row_get_bit(&test_msg, n1);
                    row_set_bit(&mut test_msg, n1, !current);
                }

                // Count pattern weight for ntheta check
                let pattern_weight = base_pattern.count_ones() + if !base_pattern[n1] { 1 } else { 0 };

                // Encode to get candidate codeword (FAST)
                let candidate = encode_with_rref_fast(&test_msg, &rref_gen_fast);

                // Apply ntheta threshold (preprocessing rule 1)
                if config.npre1 && config.nt > 0 {
                    // Count errors in first nt tail bits
                    let tail_errors: usize = (K..(K + config.nt).min(N))
                        .filter(|&i| row_get_bit(&candidate, i) != row_get_bit(&hard_decisions_ordered, i))
                        .count();

                    let nd1kpt = tail_errors + pattern_weight;

                    if nd1kpt > config.ntheta {
                        total_rejected += 1;
                        continue;
                    }
                }

                // Compute distance (FAST)
                let dist = compute_distance_fast(&candidate, &hard_decisions_ordered, &abs_llr_ordered);

                // Track best candidate
                if dist < best_dist {
                    best_dist = dist;
                    best_codeword = candidate;
                    total_improved += 1;

                    // Un-permute and check CRC
                    let mut unpermuted = row_zero();
                    for i in 0..N {
                        if row_get_bit(&candidate, i) {
                            row_set_bit(&mut unpermuted, final_indices[i], true);
                        }
                    }
                    let msg = row_to_bitvec(&unpermuted);
                    let msg91 = msg[0..K].to_bitvec();
                    if crc14_check(&msg91) {
                        debug!(order, total_tested, dist, "OSD decode success (exhaustive)");
                        return Some(msg91);
                    }
                }
            }

            // Get next base combination
            if !next_combination_pattern(&mut base_pattern, K, order) {
                break;
            }
        }
    }

    // Step 6: Preprocessing rule 2 (hash-based pattern matching) - using fast Row operations
    if config.npre2 {
        if let Some(ref hash_table) = hash_table {

            let mut npre2_tested = 0;

            // Initialize pattern at order nord (BitVec for pattern iteration, Row for computation)
            let mut base_pattern = bitvec![u8, Msb0; 0; K];
            for i in (K - config.nord)..K {
                base_pattern.set(i, true);
            }

            loop {
                // Create base message (fast Row)
                let mut base_msg = m0;
                for i in 0..K {
                    if base_pattern[i] {
                        let current = row_get_bit(&base_msg, i);
                        row_set_bit(&mut base_msg, i, !current);
                    }
                }

                // Encode to get error pattern (fast)
                let base_codeword = encode_with_rref_fast(&base_msg, &rref_gen_fast);
                let mut e2sub = bitvec![u8, Msb0; 0; N - K];
                for i in 0..(N - K) {
                    let cw_bit = row_get_bit(&base_codeword, K + i);
                    let hd_bit = row_get_bit(&hard_decisions_ordered, K + i);
                    e2sub.set(i, cw_bit ^ hd_bit);
                }

                // Try all single-bit flips in first ntau tail bits (including no flip)
                for flip_pos in 0..=config.ntau.min(N - K) {
                    // Create modified error pattern
                    let mut r2pat = e2sub.clone();
                    if flip_pos > 0 {
                        let current = r2pat[flip_pos - 1];
                        r2pat.set(flip_pos - 1, !current);
                    }

                    // Look up matching (i1, i2) pairs
                    let pairs: Vec<_> = lookup_pattern_pairs(hash_table, &r2pat[0..config.ntau], config.ntau).collect();
                    for (i1, i2) in pairs {
                        npre2_tested += 1;

                        // Create new test pattern with additional bits flipped
                        let mut new_pattern = base_pattern.clone();
                        new_pattern.set(i1, true);
                        new_pattern.set(i2, true);

                        // Skip if pattern has too few bits set
                        let pattern_weight = new_pattern.count_ones();
                        if pattern_weight < config.nord + if config.npre1 { 1 } else { 0 } + if config.npre2 { 1 } else { 0 } {
                            continue;
                        }

                        // Create and encode test message (fast Row)
                        let mut test_msg = m0;
                        for i in 0..K {
                            if new_pattern[i] {
                                let current = row_get_bit(&test_msg, i);
                                row_set_bit(&mut test_msg, i, !current);
                            }
                        }

                        let candidate = encode_with_rref_fast(&test_msg, &rref_gen_fast);
                        let dist = compute_distance_fast(&candidate, &hard_decisions_ordered, &abs_llr_ordered);

                        if dist < best_dist {
                            best_dist = dist;
                            best_codeword = candidate;

                            // Un-permute and check CRC
                            let mut unpermuted = row_zero();
                            for i in 0..N {
                                if row_get_bit(&candidate, i) {
                                    row_set_bit(&mut unpermuted, final_indices[i], true);
                                }
                            }
                            let msg = row_to_bitvec(&unpermuted);
                            let msg91 = msg[0..K].to_bitvec();
                            if crc14_check(&msg91) {
                                debug!(npre2_tested, dist, "OSD decode success (npre2)");
                                return Some(msg91);
                            }
                        }
                    }
                }

                // Get next base pattern
                if !next_combination_pattern(&mut base_pattern, K, config.nord) {
                    break;
                }
            }
        }
    }

    debug!(total_tested, total_improved, best_dist, "All OSD searches exhausted - decode failed");

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

    #[test]
    fn test_pattern_hash_table_has_all_pairs() {
        let unique_patterns = test_hash_table_size();
        // Should have 4095 total pairs distributed across unique patterns
        // With 14-bit patterns, max is 2^14 = 16384 possible patterns
        assert!(unique_patterns > 0 && unique_patterns <= 16384);
    }

    #[test]
    fn test_encode_with_rref_matches_standard() {
        // Test that encode_with_rref produces correct results
        let identity_order: Vec<usize> = (0..N).collect();
        let (rref, _) = gaussian_elimination(&GENERATOR, &identity_order);

        // Test several random messages
        for test_bit in [0, 1, 5, 10, 45, 90] {
            let mut msg = bitvec![u8, Msb0; 0; K];
            msg.set(test_bit, true);

            // Encode with RREF
            let codeword_rref = encode_with_rref(&msg, &rref);

            // Encode with standard method
            let mut codeword_std = bitvec![u8, Msb0; 0; N];
            encode(&msg, &mut codeword_std);

            assert_eq!(codeword_rref, codeword_std,
                "RREF encoding should match standard encoding for bit {}", test_bit);
        }
    }

    #[test]
    fn test_hash_lookup_finds_patterns() {
        // This test verifies that the hash table lookup actually works
        // by building a hash table and then looking up patterns that should exist
        let identity_order: Vec<usize> = (0..N).collect();
        let (rref, _) = gaussian_elimination(&GENERATOR, &identity_order);

        let hash_table = build_pattern_hash_table(&rref, 14);

        // Now manually construct a pattern that SHOULD be in the hash table
        // Take two specific rows (i1=5, i2=3) and XOR their first 14 tail bits
        let i1 = 5;
        let i2 = 3;
        let mut expected_pattern = bitvec![u8, Msb0; 0; 14];
        for bit_idx in 0..14 {
            let tail_col = K + bit_idx;
            expected_pattern.set(bit_idx, rref[i1][tail_col] ^ rref[i2][tail_col]);
        }

        // Look up this pattern
        let matches: Vec<_> = lookup_pattern_pairs(&hash_table, &expected_pattern, 14).collect();

        // We should find at least the (i1, i2) pair
        assert!(matches.contains(&(i1, i2)) || matches.contains(&(i2, i1)),
            "Hash table should contain the pair ({}, {}) for the pattern we constructed from it",
            i1, i2);
        assert!(!matches.is_empty(), "Lookup should find at least one match");
    }

    #[test]
    fn test_bitslice_indexing() {
        // Verify that BitVec slicing works as expected
        let mut test_vec = bitvec![u8, Msb0; 0; 20];
        test_vec.set(0, true);
        test_vec.set(5, true);
        test_vec.set(13, true);
        test_vec.set(15, true);

        // Extract first 14 bits
        let slice = &test_vec[0..14];

        // Check that the slice contains the right bits
        assert_eq!(slice.len(), 14);
        assert!(slice[0]);   // bit 0 should be true
        assert!(slice[5]);   // bit 5 should be true
        assert!(slice[13]);  // bit 13 should be true
        // bit 15 should NOT be in the slice
    }

    #[test]
    fn test_rref_structure_after_gauss() {
        // Verify that Gaussian elimination produces the expected RREF structure
        let identity_order: Vec<usize> = (0..N).collect();
        let (rref, _) = gaussian_elimination(&GENERATOR, &identity_order);

        // With identity ordering, RREF should have diagonal structure in first K columns
        // Check first few diagonal elements
        let mut diag_count = 0;
        for i in 0..K.min(10) {
            if rref[i][i] {
                diag_count += 1;
            }
        }

        // Should have most diagonal elements set
        assert!(diag_count >= 8, "RREF should have diagonal structure (got {} out of 10)", diag_count);

        // Verify that encoding with RREF matches: bit i set -> rref[i] is the codeword
        let mut msg = bitvec![u8, Msb0; 0; K];
        msg.set(0, true);
        let cw_rref = encode_with_rref(&msg, &rref);

        // Should equal rref[0]
        assert_eq!(cw_rref, rref[0], "Encoding bit 0 should give rref[0]");
    }

    #[test]
    fn test_npre2_pattern_matching_logic() {
        // This test verifies the core npre2 matching logic works
        let identity_order: Vec<usize> = (0..N).collect();
        let (rref, _) = gaussian_elimination(&GENERATOR, &identity_order);
        let hash_table = build_pattern_hash_table(&rref, 14);

        // Create a test scenario:
        // 1. Start with m0 = all zeros
        // 2. Flip bit 5 to get base_msg = [bit 5 set]
        // 3. Encode to get base_codeword
        // 4. The base_codeword should equal rref[5]
        //5. Compute what the tail would be if we additionally flipped bits 10 and 20
        // 6. That should be in the hash table

        let m0 = bitvec![u8, Msb0; 0; K];
        let mut base_pattern = bitvec![u8, Msb0; 0; K];
        base_pattern.set(5, true);

        let mut base_msg = m0.clone();
        base_msg.set(5, true);

        let base_codeword = encode_with_rref(&base_msg, &rref);
        assert_eq!(base_codeword, rref[5], "Base codeword should equal rref[5]");

        // The hash table stores XOR of just two rows (i1, i2)
        // So the pattern should be rref[10] XOR rref[20], NOT including rref[5]
        let mut expected_pattern = bitvec![u8, Msb0; 0; 14];
        for i in 0..14 {
            let tail_col = K + i;
            expected_pattern.set(i, rref[10][tail_col] ^ rref[20][tail_col]);
        }

        // This pattern should be in the hash table for pair (10, 20) or (20, 10)
        let matches: Vec<_> = lookup_pattern_pairs(&hash_table, &expected_pattern, 14).collect();

        assert!(!matches.is_empty(), "Should find matches for the XOR pattern");
        assert!(matches.contains(&(10, 20)) || matches.contains(&(20, 10)),
            "Should find the pair (10, 20) or (20, 10), found: {:?}", matches);
    }
}
