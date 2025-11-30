//! LDPC decoder using belief propagation (sum-product algorithm)

use bitvec::prelude::*;
use bitvec::vec::BitVec;
use crate::crc::crc14_check;
use super::constants::*;

/// Piecewise linear approximation of atanh used by WSJT-X
///
/// This is NOT the mathematical atanh function! WSJT-X uses a piecewise
/// linear approximation that has been tuned for LDPC decoding performance.
/// The approximation differs by 10-40% from mathematical atanh in typical
/// operating ranges, and caps output at ±7.0 for numerical stability.
///
/// The function uses 5 linear segments:
/// - |x| ≤ 0.664: y = x / 0.83
/// - 0.664 < |x| ≤ 0.9217: y = sign(x) * (|x| - 0.4064) / 0.322
/// - 0.9217 < |x| ≤ 0.9951: y = sign(x) * (|x| - 0.8378) / 0.0524
/// - 0.9951 < |x| ≤ 0.9998: y = sign(x) * (|x| - 0.9914) / 0.0012
/// - |x| > 0.9998: y = sign(x) * 7.0
///
/// Reference: wsjtx/lib/platanh.f90
#[inline]
fn platanh(x: f32) -> f32 {
    let isign = if x < 0.0 { -1.0 } else { 1.0 };
    let z = x.abs();

    if z <= 0.664 {
        x / 0.83
    } else if z <= 0.9217 {
        isign * (z - 0.4064) / 0.322
    } else if z <= 0.9951 {
        isign * (z - 0.8378) / 0.0524
    } else if z <= 0.9998 {
        isign * (z - 0.9914) / 0.0012
    } else {
        isign * 7.0
    }
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
/// * `Some((message, iterations, nharderrors))` - Decoded message, BP iteration count, and initial hard error count
/// * `None` - If decoding failed (max iterations reached or no valid codeword found)
///
/// The decoder uses the sum-product algorithm (belief propagation) to iteratively
/// refine bit estimates by passing messages between bit nodes and check nodes.
/// Decoding succeeds when all parity checks are satisfied AND the CRC is valid.
pub fn decode(llr: &[f32], max_iterations: usize) -> Option<(BitVec<u8, Msb0>, usize, usize)> {
    decode_with_ap(llr, None, max_iterations)
}

/// LDPC BP decoder with optional AP (a priori) mask
///
/// If `apmask` is provided, bits marked as `true` in the mask will not participate
/// in BP message passing - they remain fixed at their LLR hint values.
pub fn decode_with_ap(
    llr: &[f32],
    apmask: Option<&[bool]>,
    max_iterations: usize
) -> Option<(BitVec<u8, Msb0>, usize, usize)> {
    if llr.len() != N {
        return None;
    }

    // Validate AP mask if provided
    if let Some(mask) = apmask {
        if mask.len() != N {
            return None;
        }
    }

    // Compute initial LLR quality metrics for debugging
    let _llr_mean = llr.iter().map(|x| x.abs()).sum::<f32>() / llr.len() as f32;
    let _llr_max = llr.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);

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

    // Track initial hard errors (nharderrors) from channel LLRs (before BP starts)
    let mut nharderrors = 0usize;

    // Iterative decoding
    for iter in 0..=max_iterations {
        // Update bit log-likelihood ratios
        // CRITICAL: AP-masked bits don't get updated - they stay fixed!
        for i in 0..N {
            if let Some(mask) = apmask {
                if mask[i] {
                    // AP bit: frozen at hint value (no BP update)
                    zn[i] = llr[i];
                } else {
                    // Normal bit: regular BP update
                    zn[i] = llr[i] + tov[i].iter().sum::<f32>();
                }
            } else {
                // No AP mask: all bits get normal BP update
                zn[i] = llr[i] + tov[i].iter().sum::<f32>();
            }
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

        // Save initial hard error count (before BP starts, at iteration 0)
        if iter == 0 {
            nharderrors = ncheck;
        }

        // If all parity checks satisfied, check CRC
        if ncheck == 0 {
            let decoded = &cw[..K];
            if crc14_check(decoded) {
                // Success! Return the decoded message and metrics
                // eprintln!("    BP CONVERGED at iteration {} (CRC valid)", iter);
                return Some((decoded.to_bitvec(), iter, nharderrors));
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

                // Apply platanh (WSJT-X's piecewise linear approximation) to get the message
                tov[j][i] = 2.0 * platanh(-product);
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
/// * `Ok((message, iterations, nharderrors, snapshots))` - Decoded message, BP iteration count, initial hard errors, and saved LLR snapshots
/// * `Err(snapshots)` - If BP failed, returns the saved LLR snapshots for OSD fallback
pub fn decode_with_snapshots(
    llr: &[f32],
    max_iterations: usize,
    save_at_iters: &[usize],
) -> Result<(BitVec<u8, Msb0>, usize, usize, Vec<Vec<f32>>), Vec<Vec<f32>>> {
    if llr.len() != N {
        return Err(Vec::new());
    }

    // Message arrays
    let mut toc = vec![vec![0.0f32; MAX_NRW]; M]; // Messages to checks
    let mut tov = vec![vec![0.0f32; NCW]; N];     // Messages to variable nodes
    let mut zn = vec![0.0f32; N];                  // Bit log-likelihood estimates

    // Storage for LLR snapshots
    let mut snapshots: Vec<Vec<f32>> = Vec::new();

    // Track initial hard errors (nharderrors) from channel LLRs (before BP starts)
    let mut nharderrors = 0usize;

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

        // Save initial hard error count (before BP starts, at iteration 0)
        if iter == 0 {
            nharderrors = ncheck;
        }

        // If all parity checks satisfied, check CRC
        if ncheck == 0 {
            let decoded = &cw[..K];
            if crc14_check(decoded) {
                // Success! Return the decoded message with metrics and any snapshots collected so far
                return Ok((decoded.to_bitvec(), iter, nharderrors, snapshots));
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

                // Apply platanh (WSJT-X's piecewise linear approximation) to get the message
                tov[j][i] = 2.0 * platanh(-product);
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
        let (decoded, iterations, _errors) = result.unwrap();
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
        let (decoded, _iterations, _errors) = result.unwrap();

        // Verify the decoded message matches the original (before errors)
        for i in 0..77 {
            assert_eq!(decoded[i], msg_str.chars().nth(i).unwrap() == '1');
        }
    }

    /// Test LDPC decoder with real-world LLR values from WSJT-X
    ///
    /// This test uses pre-decode LLR values extracted from WSJT-X's ft8b.f90
    /// for the message "N1PJT HB9CQK -10" at -10 dB SNR.
    ///
    /// The LLR values were extracted BEFORE LDPC error correction, right after
    /// symbol demodulation. WSJT-X successfully decodes this message with 20
    /// hard errors corrected by LDPC.
    ///
    /// Source: tests/test_data/210703_133430.wav at 465.625 Hz, time offset 0.75s
    /// Modified Fortran source: tests/sync/ft8b_llr_extract.f90
    ///
    /// This test is currently expected to FAIL - it serves as a benchmark for
    /// improving the LDPC decoder to match WSJT-X's performance.
    #[test]
    #[ignore] // Remove #[ignore] once LDPC decoder is improved
    fn test_decode_real_wsjt_x_llr_n1pjt_hb9cqk() {
        // Pre-decode LLR values from WSJT-X (before LDPC correction)
        // Frequency: 465.625 Hz, time offset: 0.75s, sync: 1.79e+09
        // SNR: -10 dB
        let llr: Vec<f32> = vec![
            -2.614, 1.750, -2.774, 2.361, -2.361, 2.361, 4.480, 2.092, 2.092, 1.677,
            -1.675, 1.677, -0.938, -0.938, -0.938, -3.427, 3.317, 3.533, -3.359, -1.968,
            -3.417, 3.421, 1.874, -1.874, 1.709, 1.853, 1.964, 2.468, -2.344, 2.344,
            -0.471, -2.309, -0.471, -1.426, -1.426, -0.750, 1.397, 0.107, 1.397, -2.459,
            2.622, -2.622, 2.025, 1.079, 1.876, -0.492, 0.492, 0.492, 0.186, 2.169,
            0.186, 2.068, 0.803, 0.803, 1.394, 2.408, -1.394, -0.716, -1.858, 1.858,
            1.192, 2.985, 1.192, 2.746, 2.064, -2.064, 3.776, -2.896, 1.634, 0.265,
            -0.265, 0.265, -1.545, 1.545, -1.545, -1.253, -0.353, 1.253, 1.245, 2.141,
            -1.245, 2.407, -2.407, -2.407, 3.042, 1.642, 1.642, 3.754, 3.639, -3.825,
            2.849, 2.436, -2.436, 1.871, 2.428, -1.871, -1.599, -1.599, -2.075, 1.628,
            -1.684, -1.783, -1.527, -0.883, 0.883, 0.599, 0.599, -0.599, -1.718, -1.718,
            -1.718, -2.560, 2.901, -2.813, 3.306, 2.579, 3.271, 4.096, -3.763, -3.763,
            -4.323, -4.323, 4.323, 3.633, 4.194, -3.633, 4.588, -4.700, 4.581, 3.792,
            -3.629, -4.319, 2.415, 3.128, 2.415, 1.634, -1.634, 1.634, 1.350, -0.784,
            -1.350, 0.371, 0.371, -0.371, -0.947, 1.291, 0.265, 1.799, 1.799, 1.799,
            -3.115, -3.177, -2.993, -3.635, 3.635, -3.635, -4.903, 4.903, -4.903, 5.170,
            4.648, 5.057, 5.760, -5.760, 5.760, 6.260, 6.173, 4.425, -5.373, -5.030,
            5.373, -5.785, 5.785, -5.916,
        ];

        // Expected decoded message bits (from WSJT-X successful decode)
        let expected_bits: Vec<u8> = vec![
            0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0,
            0, 1, 1, 0, 1, 1, 1, 1, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0,
            1, 0, 1, 1, 1, 0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 1,
            1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1,
        ];

        // Expected message text
        let expected_message = "N1PJT HB9CQK -10";

        println!("\n=== Testing LDPC Decode for Real WSJT-X Signal ===");
        println!("Message: {}", expected_message);
        println!("SNR: -10 dB");
        println!("LLR stats: mean={:.2}, max={:.2}",
            llr.iter().map(|x| x.abs()).sum::<f32>() / llr.len() as f32,
            llr.iter().map(|x| x.abs()).fold(0.0f32, f32::max));

        // Decode using hybrid BP+OSD strategy like WSJT-X (BP first, then OSD fallback)
        // This uses DecodeDepth::BpOsdHybrid which matches WSJT-X's maxosd=2
        use crate::ldpc::{decode_hybrid, DecodeDepth};
        println!("Calling decode_hybrid with BpOsdHybrid...");
        let result = decode_hybrid(&llr, DecodeDepth::BpOsdHybrid);
        println!("decode_hybrid returned: {}", if result.is_some() { "Some" } else { "None" });

        match result {
            Some((decoded_bits, iterations, errors_corrected)) => {
                println!("✓ LDPC decode succeeded!");
                println!("  Iterations: {}", iterations);
                println!("  Errors corrected: {}", errors_corrected);

                // Convert BitVec to Vec<u8> for comparison
                let message: Vec<u8> = decoded_bits.iter().by_vals().take(77)
                    .map(|b| if b { 1 } else { 0 })
                    .collect();

                let matching_bits: usize = message.iter()
                    .zip(expected_bits.iter())
                    .filter(|(a, b)| a == b)
                    .count();

                println!("  Bit accuracy: {}/{} ({:.1}%)",
                    matching_bits, expected_bits.len(),
                    100.0 * matching_bits as f32 / expected_bits.len() as f32);

                // Verify exact match
                assert_eq!(message, expected_bits,
                    "Decoded bits should match WSJT-X output exactly");

                println!("\n✓ Test passed! RustyFt8 LDPC decoder matches WSJT-X performance.");
            }
            None => {
                panic!("\nLDPC decode failed for real WSJT-X signal.\n\
                       WSJT-X successfully decoded this -10 dB message with 20 hard errors.\n\
                       This indicates the LDPC decoder needs improvement.\n\
                       LLR values are from: tests/test_data/210703_133430.wav");
            }
        }
    }
}
