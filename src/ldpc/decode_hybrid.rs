//! Hybrid BP/OSD decoder with LLR snapshots (matches WSJT-X decode174_91.f90)

use bitvec::prelude::*;
use bitvec::vec::BitVec;
use crate::crc::crc14_check;
use super::constants::*;
use super::platanh::platanh;
use super::decode_bp::MAX_BP_ITERATIONS;

/// Decode with LLR snapshots saved at specified iterations
///
/// This is the hybrid BP/OSD decoder strategy used by WSJT-X.
/// During BP iterations, we accumulate LLRs (zsum = zsum + zn) and save
/// snapshots at specific iterations. If BP fails to converge, these
/// accumulated snapshots provide smoothed soft information for OSD fallback.
///
/// Matches WSJT-X's decode174_91.f90 loop structure:
/// - Loop starts at iter=1, tov/toc initialized to 0
/// - At iter=1: zn = llr (since tov=0), zsum = llr
/// - Snapshots at iter=1,2,3 match WSJT-X's llr1(:,1:3)
///
/// # Arguments
/// * `llr` - Log-Likelihood Ratios for each of 174 bits
/// * `save_at_iters` - Which iterations to save accumulated LLR snapshots (e.g., [1, 2, 3])
///
/// # Returns
/// * `Ok((message, iterations, nharderrors, snapshots))` - Decoded message, BP iteration count, initial hard errors, and saved LLR snapshots
/// * `Err(snapshots)` - If BP failed, returns the saved accumulated LLR snapshots for OSD fallback
pub(super) fn decode_with_snapshots(
    llr: &[f32],
    save_at_iters: &[usize],
) -> Result<(BitVec<u8, Msb0>, usize, usize, Vec<Vec<f32>>), Vec<Vec<f32>>> {
    if llr.len() != N {
        return Err(Vec::new());
    }

    // Message arrays - initialized to 0 (matching WSJT-X: tov=0, toc=0)
    let mut toc = vec![vec![0.0f32; MAX_NRW]; M]; // Messages to checks
    let mut tov = vec![vec![0.0f32; NCW]; N];     // Messages to variable nodes
    let mut zn = vec![0.0f32; N];                  // Bit log-likelihood estimates
    let mut zsum = vec![0.0f32; N];               // Accumulated LLRs (WSJT-X: zsum=0)

    // Storage for LLR snapshots
    let mut snapshots: Vec<Vec<f32>> = Vec::new();

    // Track initial hard errors and early stopping
    let mut nharderrors = 0usize;
    let mut nclast = 0usize;
    let mut ncnt = 0usize;

    // Iterative decoding - start at iter=1 to match WSJT-X
    // At iter=1, tov is still 0, so zn = llr
    for iter in 1..=MAX_BP_ITERATIONS {
        // Update bit log-likelihood ratios
        for i in 0..N {
            zn[i] = llr[i] + tov[i].iter().sum::<f32>();
        }

        // Accumulate LLRs (WSJT-X: zsum = zsum + zn)
        for i in 0..N {
            zsum[i] += zn[i];
        }

        // Save ACCUMULATED snapshot at requested iterations (WSJT-X saves zsum, not zn!)
        if save_at_iters.contains(&iter) {
            snapshots.push(zsum.clone());
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
                // Compute nharderrors = bit flips from initial LLR (matching WSJT-X)
                nharderrors = 0;
                for i in 0..N {
                    let cw_sign = if cw[i] { 1.0f32 } else { -1.0f32 };
                    if cw_sign * llr[i] < 0.0 {
                        nharderrors += 1;
                    }
                }
                return Ok((decoded.to_bitvec(), iter, nharderrors, snapshots));
            }
        }

        // Early stopping criterion (matching WSJT-X: iter.gt.1)
        if iter > 1 {
            if ncheck >= nclast {
                ncnt += 1;
            } else {
                ncnt = 0;
            }
            if ncnt >= 5 && iter >= 10 && ncheck > 15 {
                return Err(snapshots);
            }
        }
        nclast = ncheck;

        // If we've reached max iterations, return snapshots for OSD fallback
        if iter == MAX_BP_ITERATIONS {
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
