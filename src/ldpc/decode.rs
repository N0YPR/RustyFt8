//! LDPC decoder using belief propagation (sum-product algorithm)

use bitvec::prelude::*;
use bitvec::vec::BitVec;
use crate::crc::crc14_check;
use super::constants::*;

/// Safe implementation of atanh with clipping to avoid NaN/infinity
#[inline]
fn atanh_safe(x: f32) -> f32 {
    // Clip x to valid range (-1, 1) with small margin
    let x_clipped = x.clamp(-0.999999, 0.999999);
    0.5 * f32::ln((1.0 + x_clipped) / (1.0 - x_clipped))
}

/// Decode a 174-bit codeword using LDPC(174,91) belief propagation
///
/// Takes soft information (Log-Likelihood Ratios) for a received 174-bit codeword
/// and attempts to decode it to the original 91-bit message (77 info + 14 CRC bits).
///
/// # Arguments
/// * `llr` - Log-Likelihood Ratios for each of 174 bits
///   - Positive values indicate confidence the bit is 1
///   - Negative values indicate confidence the bit is 0
///   - Magnitude indicates confidence level
/// * `max_iterations` - Maximum number of decoding iterations (typically 20-50)
///
/// # Returns
/// * `Some((message, iterations))` - Decoded 91-bit message and iteration count if successful
/// * `None` - If decoding failed (max iterations reached or no valid codeword found)
///
/// The decoder uses the sum-product algorithm (belief propagation) to iteratively
/// refine bit estimates by passing messages between bit nodes and check nodes.
/// Decoding succeeds when all parity checks are satisfied AND the CRC is valid.
pub fn decode(llr: &[f32], max_iterations: usize) -> Option<(BitVec<u8, Msb0>, usize)> {
    if llr.len() != N {
        return None;
    }

    // Compute initial LLR quality metrics for debugging
    let llr_mean = llr.iter().map(|x| x.abs()).sum::<f32>() / llr.len() as f32;
    let llr_max = llr.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);

    // Message arrays
    let mut toc = vec![vec![0.0f32; MAX_NRW]; M]; // Messages to checks
    let mut tov = vec![vec![0.0f32; NCW]; N];     // Messages to variable nodes
    let mut zn = vec![0.0f32; N];                  // Bit log-likelihood estimates

    // Initialize messages to checks with LLRs
    for j in 0..M {
        for i in 0..NRW[j] {
            let bit_idx = NM[j][i];
            toc[j][i] = llr[bit_idx];
        }
    }

    // Iterative decoding
    for iter in 0..=max_iterations {
        // Update bit log-likelihood ratios
        for i in 0..N {
            zn[i] = llr[i] + tov[i].iter().sum::<f32>();
        }

        // Make hard decisions
        let mut cw = BitVec::<u8, Msb0>::repeat(false, N);
        for i in 0..N {
            cw.set(i, zn[i] > 0.0);
        }

        // Check parity constraints
        let mut ncheck = 0;
        for i in 0..M {
            let mut parity = 0u8;
            for j in 0..NRW[i] {
                let bit_idx = NM[i][j];
                if cw[bit_idx] {
                    parity ^= 1;
                }
            }
            if parity != 0 {
                ncheck += 1;
            }
        }

        // Log progress every 10 iterations or at key points
        // if iter == 0 || iter == 10 || iter == 20 || iter == max_iterations || ncheck == 0 {
        //     eprintln!("    BP iter {}: ncheck={}/83, llr_mean={:.2}, llr_max={:.2}",
        //              iter, ncheck, llr_mean, llr_max);
        // }

        // If all parity checks satisfied, check CRC
        if ncheck == 0 {
            let decoded = &cw[..K];
            if crc14_check(decoded) {
                // Success! Return the decoded message
                // eprintln!("    BP CONVERGED at iteration {} (CRC valid)", iter);
                return Some((decoded.to_bitvec(), iter));
            } else {
                // eprintln!("    BP iter {}: All parity OK but CRC FAILED", iter);
            }
        }

        // If we've reached max iterations, give up
        if iter == max_iterations {
            // eprintln!("    BP FAILED: max_iters={}, final ncheck={}/83", max_iterations, ncheck);
            break;
        }

        // Send messages from bits to check nodes
        for j in 0..M {
            for i in 0..NRW[j] {
                let bit_idx = NM[j][i];
                toc[j][i] = zn[bit_idx];

                // Subtract off what the bit had received from this check
                for kk in 0..NCW {
                    if MN[bit_idx][kk] == j {
                        toc[j][i] -= tov[bit_idx][kk];
                        break;
                    }
                }
            }
        }

        // Send messages from check nodes to variable nodes
        // This is the core of the sum-product algorithm
        for j in 0..N {
            for i in 0..NCW {
                let check_idx = MN[j][i];

                // Compute product of tanh(-toc/2) for all bits in check except j
                let mut product = 1.0f32;
                for k in 0..NRW[check_idx] {
                    let bit_k = NM[check_idx][k];
                    if bit_k != j {
                        product *= f32::tanh(-toc[check_idx][k] / 2.0);
                    }
                }

                // Apply atanh to get the message
                tov[j][i] = 2.0 * atanh_safe(-product);
            }
        }
    }

    None
}

/// Decode with LLR snapshots saved at specified iterations
///
/// This is the hybrid BP/OSD decoder strategy used by WSJT-X.
/// During BP iterations, we save LLR states at specific iterations (typically 1, 2, 3).
/// If BP fails to converge, these snapshots can be used with OSD for multiple attempts.
///
/// # Arguments
/// * `llr` - Log-Likelihood Ratios for each of 174 bits
/// * `max_iterations` - Maximum number of BP iterations (typically 30)
/// * `save_at_iters` - Which iterations to save LLR snapshots (e.g., [1, 2, 3])
///
/// # Returns
/// * `Ok((message, iterations, snapshots))` - Decoded message, iteration count, and saved LLR snapshots
/// * `Err(snapshots)` - If BP failed, returns the saved LLR snapshots for OSD fallback
pub fn decode_with_snapshots(
    llr: &[f32],
    max_iterations: usize,
    save_at_iters: &[usize],
) -> Result<(BitVec<u8, Msb0>, usize, Vec<Vec<f32>>), Vec<Vec<f32>>> {
    if llr.len() != N {
        return Err(Vec::new());
    }

    // Message arrays
    let mut toc = vec![vec![0.0f32; MAX_NRW]; M]; // Messages to checks
    let mut tov = vec![vec![0.0f32; NCW]; N];     // Messages to variable nodes
    let mut zn = vec![0.0f32; N];                  // Bit log-likelihood estimates

    // Storage for LLR snapshots
    let mut snapshots: Vec<Vec<f32>> = Vec::new();

    // Initialize messages to checks with LLRs
    for j in 0..M {
        for i in 0..NRW[j] {
            let bit_idx = NM[j][i];
            toc[j][i] = llr[bit_idx];
        }
    }

    // Iterative decoding
    for iter in 0..=max_iterations {
        // Update bit log-likelihood ratios
        for i in 0..N {
            zn[i] = llr[i] + tov[i].iter().sum::<f32>();
        }

        // Save snapshot at requested iterations (WSJT-X saves at 1, 2, 3)
        if iter > 0 && save_at_iters.contains(&iter) {
            snapshots.push(zn.clone());
        }

        // Make hard decisions
        let mut cw = BitVec::<u8, Msb0>::repeat(false, N);
        for i in 0..N {
            cw.set(i, zn[i] > 0.0);
        }

        // Check parity constraints
        let mut ncheck = 0;
        for i in 0..M {
            let mut parity = 0u8;
            for j in 0..NRW[i] {
                let bit_idx = NM[i][j];
                if cw[bit_idx] {
                    parity ^= 1;
                }
            }
            if parity != 0 {
                ncheck += 1;
            }
        }

        // If all parity checks satisfied, check CRC
        if ncheck == 0 {
            let decoded = &cw[..K];
            if crc14_check(decoded) {
                // Success! Return the decoded message with any snapshots collected so far
                return Ok((decoded.to_bitvec(), iter, snapshots));
            }
        }

        // If we've reached max iterations, return snapshots for OSD fallback
        if iter == max_iterations {
            return Err(snapshots);
        }

        // Send messages from bits to check nodes
        for j in 0..M {
            for i in 0..NRW[j] {
                let bit_idx = NM[j][i];
                toc[j][i] = zn[bit_idx];

                // Subtract off what the bit had received from this check
                for kk in 0..NCW {
                    if MN[bit_idx][kk] == j {
                        toc[j][i] -= tov[bit_idx][kk];
                        break;
                    }
                }
            }
        }

        // Send messages from check nodes to variable nodes
        // This is the core of the sum-product algorithm
        for j in 0..N {
            for i in 0..NCW {
                let check_idx = MN[j][i];

                // Compute product of tanh(-toc/2) for all bits in check except j
                let mut product = 1.0f32;
                for k in 0..NRW[check_idx] {
                    let bit_k = NM[check_idx][k];
                    if bit_k != j {
                        product *= f32::tanh(-toc[check_idx][k] / 2.0);
                    }
                }

                // Apply atanh to get the message
                tov[j][i] = 2.0 * atanh_safe(-product);
            }
        }
    }

    Err(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ldpc_decode_perfect_codeword() {
        // Test with a perfect codeword (no errors)
        // Using the known test message "CQ SOTA N0YPR/R DM42"

        // First encode a known message
        let msg_str = "00000000010111100101100110000000010100100110110011100110110001100111110010001";
        let crc_str = "00001001100101";
        let parity_str = "11100110011001101100100111100011101000010001100111111001100110001110011001011110010";

        let mut codeword_storage = [0u8; 22];
        let codeword_bits = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];

        // Fill in the codeword
        for (i, c) in msg_str.chars().enumerate() {
            codeword_bits.set(i, c == '1');
        }
        for (i, c) in crc_str.chars().enumerate() {
            codeword_bits.set(77 + i, c == '1');
        }
        for (i, c) in parity_str.chars().enumerate() {
            codeword_bits.set(91 + i, c == '1');
        }

        // Convert to LLRs (high confidence: +/-10)
        let mut llr = vec![0.0f32; 174];
        for i in 0..174 {
            llr[i] = if codeword_bits[i] { 10.0 } else { -10.0 };
        }

        // Decode
        let result = decode(&llr, 50);

        // Should succeed on first iteration (iter=0) since it's perfect
        assert!(result.is_some());
        let (decoded, iterations) = result.unwrap();
        assert_eq!(decoded.len(), 91);
        assert_eq!(iterations, 0); // Should decode immediately

        // Verify the decoded message matches the input
        for i in 0..77 {
            assert_eq!(decoded[i], msg_str.chars().nth(i).unwrap() == '1');
        }
    }

    #[test]
    fn test_ldpc_decode_with_errors() {
        // Test with a codeword that has a few bit errors
        let msg_str = "00000000010111100101100110000000010100100110110011100110110001100111110010001";
        let crc_str = "00001001100101";
        let parity_str = "11100110011001101100100111100011101000010001100111111001100110001110011001011110010";

        let mut codeword_storage = [0u8; 22];
        let codeword_bits = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];

        for (i, c) in msg_str.chars().enumerate() {
            codeword_bits.set(i, c == '1');
        }
        for (i, c) in crc_str.chars().enumerate() {
            codeword_bits.set(77 + i, c == '1');
        }
        for (i, c) in parity_str.chars().enumerate() {
            codeword_bits.set(91 + i, c == '1');
        }

        // Introduce some errors in parity bits
        codeword_bits.set(100, !codeword_bits[100]);
        codeword_bits.set(120, !codeword_bits[120]);

        // Convert to LLRs with moderate confidence
        let mut llr = vec![0.0f32; 174];
        for i in 0..174 {
            llr[i] = if codeword_bits[i] { 4.0 } else { -4.0 };
        }

        // Decode
        let result = decode(&llr, 50);

        // Should successfully correct the errors
        assert!(result.is_some());
        let (decoded, _iterations) = result.unwrap();

        // Verify the decoded message matches the original (before errors)
        for i in 0..77 {
            assert_eq!(decoded[i], msg_str.chars().nth(i).unwrap() == '1');
        }
    }
}
