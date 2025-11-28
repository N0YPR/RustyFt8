//! LDPC (Low-Density Parity Check) Error Correction for FT8
//!
//! This module implements the LDPC(174,91) encoding and decoding used in FT8.
//!
//! **Encoding**: Takes a 91-bit message (77 information bits + 14 CRC bits) and
//! produces a 174-bit codeword by adding 83 parity bits.
//!
//! **Decoding**: Uses belief propagation (sum-product algorithm) to decode
//! received codewords with soft information (LLRs) back to the original message.
//!
//! The encoding uses a generator matrix to compute parity bits through
//! matrix multiplication in GF(2) (binary field).

mod constants;
mod encode;
mod decode;
mod osd;

use bitvec::prelude::*;
use constants::{NM, NRW, M};

pub use encode::encode;
pub use decode::{decode, decode_with_snapshots, decode_with_ap};
pub use osd::osd_decode;

/// Decoding depth strategy (matches WSJT-X ndepth/maxosd settings)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeDepth {
    /// BP only (maxosd=-1): Fastest, fewest false positives
    BpOnly,
    /// BP + OSD with channel LLRs only (maxosd=0): Moderate aggression
    BpOsdUncoupled,
    /// BP + OSD with BP snapshots (maxosd=2): Most aggressive, for strong candidates
    BpOsdHybrid,
}

/// Compute initial hard errors from channel LLRs (before any decoding)
///
/// This is used for WSJT-X's nharderrors metric which filters false positives.
/// Makes hard decisions directly from LLRs and counts parity check violations.
fn compute_nharderrors(llr: &[f32]) -> usize {
    if llr.len() != 174 {
        return 83; // Return maximum if invalid
    }

    // Make hard decisions from LLRs
    let mut cw = BitVec::<u8, Msb0>::repeat(false, 174);
    for i in 0..174 {
        cw.set(i, llr[i] > 0.0);
    }

    // Count parity check violations
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

    ncheck
}

/// Hybrid BP/OSD decoder matching WSJT-X's strategy
///
/// This implements WSJT-X's decode174_91.f90 approach with configurable depth:
///
/// **BpOnly** (maxosd=-1):
/// - Run BP for 30 iterations only
/// - No OSD fallback
/// - Fastest, fewest false positives
///
/// **BpOsdUncoupled** (maxosd=0):
/// - Run BP for 30 iterations
/// - If BP fails, try OSD with channel LLRs only
/// - Moderate aggression
///
/// **BpOsdHybrid** (maxosd=2):
/// - Run BP for 30 iterations, saving snapshots at iterations 1, 2, 3
/// - If BP fails, try OSD with each snapshot
/// - Most aggressive, explores different solution space regions
///
/// # Arguments
/// * `llr` - Log-Likelihood Ratios for 174 bits
/// * `depth` - Decoding depth strategy
///
/// # Returns
/// * `Some((message91, iterations, nharderrors))` - Decoded message, BP iteration count, and initial hard error count
/// * `None` - If all decode attempts failed
pub fn decode_hybrid(llr: &[f32], depth: DecodeDepth) -> Option<(BitVec<u8, Msb0>, usize, usize)> {
    decode_hybrid_with_ap(llr, None, depth)
}

/// Hybrid BP/OSD decoder with optional AP (a priori) mask
///
/// Same as `decode_hybrid` but accepts an AP mask for forced bit hints.
/// If `apmask` is provided, bits marked as `true` in the mask will not participate
/// in BP message passing - they remain fixed at their LLR hint values.
///
/// # Arguments
/// * `llr` - Log-Likelihood Ratios for 174 bits (with AP hints already applied)
/// * `apmask` - Optional boolean mask marking which bits are AP-forced
/// * `depth` - Decoding depth strategy
///
/// # Returns
/// * `Some((message91, iterations, nharderrors))` - Decoded message, BP iteration count, and initial hard error count
/// * `None` - If all decode attempts failed
pub fn decode_hybrid_with_ap(
    llr: &[f32],
    apmask: Option<&[bool]>,
    depth: DecodeDepth
) -> Option<(BitVec<u8, Msb0>, usize, usize)> {
    let max_bp_iters = 50; // Increased from 30 to give BP more chances to converge
    let osd_order = 4; // Increased from 2 to handle signals with more bit errors

    match depth {
        DecodeDepth::BpOnly => {
            // BP only, no OSD fallback (fastest, fewest false positives)
            decode_with_ap(llr, apmask, max_bp_iters)
        }

        DecodeDepth::BpOsdUncoupled => {
            // Try BP first (no snapshots needed)
            if let Some(result) = decode_with_ap(llr, apmask, max_bp_iters) {
                return Some(result);
            }

            // BP failed, try OSD with channel LLRs only
            // Note: OSD doesn't use AP mask, it works on raw LLRs
            if let Some(decoded) = osd_decode(llr, osd_order) {
                // OSD succeeded - compute nharderrors from channel LLRs
                let nharderrors = compute_nharderrors(llr);
                return Some((decoded, 0, nharderrors)); // iters=0 indicates OSD decode
            }

            None
        }

        DecodeDepth::BpOsdHybrid => {
            // If AP mask is provided, use simpler strategy (no snapshots yet)
            // TODO: Add AP mask support to decode_with_snapshots for full hybrid strategy
            if apmask.is_some() {
                // Try BP with AP first
                if let Some(result) = decode_with_ap(llr, apmask, max_bp_iters) {
                    return Some(result);
                }

                // BP+AP failed, try OSD with the AP-hinted LLRs
                if let Some(decoded) = osd_decode(llr, osd_order) {
                    let nharderrors = compute_nharderrors(llr);
                    return Some((decoded, 0, nharderrors));
                }

                return None;
            }

            // Full hybrid strategy with BP snapshots (no AP)
            let save_at_iters = [1, 2, 3];

            // Try BP first with snapshot saving
            match decode_with_snapshots(llr, max_bp_iters, &save_at_iters) {
                Ok((decoded, iters, nharderrors, _snapshots)) => {
                    // BP converged!
                    return Some((decoded, iters, nharderrors));
                }
                Err(snapshots) => {
                    // BP failed, compute nharderrors once from channel LLRs
                    let nharderrors = compute_nharderrors(llr);

                    // Try OSD with each saved snapshot
                    for (idx, snapshot_llr) in snapshots.iter().enumerate() {
                        if let Some(decoded) = osd_decode(snapshot_llr, osd_order) {
                            eprintln!("  OSD succeeded with iteration {} LLRs (order {})",
                                      save_at_iters[idx], osd_order);
                            return Some((decoded, 0, nharderrors));
                        }
                    }

                    // All snapshot attempts failed, fall back to channel LLRs
                    if let Some(decoded) = osd_decode(llr, osd_order) {
                        eprintln!("  OSD succeeded with channel LLRs (order {})", osd_order);
                        return Some((decoded, 0, nharderrors));
                    }
                }
            }

            None
        }
    }
}
