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
pub use decode::{decode, decode_with_snapshots, decode_with_ap, decode_damped};
pub use osd::{osd_decode, osd_decode_wsjt};

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
/// - If BP fails, try damped BP with various parameters
/// - If still failing, try OSD with BP snapshots
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
    let max_bp_iters = 30; // Standard BP iterations (WSJT-X uses 30)

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

            // BP failed - check if OSD is worth trying
            let nharderrors = compute_nharderrors(llr);
            if nharderrors > 36 {
                // Too many hard errors - OSD won't help, skip it
                return None;
            }

            // Try OSD order-1 (91 patterns, very fast)
            if let Some(decoded) = osd_decode(llr, 1) {
                return Some((decoded, 0, nharderrors));
            }

            None
        }

        DecodeDepth::BpOsdHybrid => {
            // Try BP first, capturing accumulated LLR snapshots for OSD fallback
            // WSJT-X saves accumulated LLRs (zsum = zsum + zn) at iterations 1, 2, 3
            let save_at_iters = [1, 2, 3];
            let bp_snapshots: Vec<Vec<f32>>;

            if apmask.is_some() {
                if let Some(result) = decode_with_ap(llr, apmask, max_bp_iters) {
                    return Some(result);
                }
                bp_snapshots = Vec::new(); // No snapshots with AP mask
            } else {
                // Try BP with snapshots
                match decode_with_snapshots(llr, max_bp_iters, &save_at_iters) {
                    Ok((decoded, iters, nharderrors, _)) => {
                        return Some((decoded, iters, nharderrors));
                    }
                    Err(snapshots) => {
                        bp_snapshots = snapshots; // Captured accumulated LLRs for OSD
                    }
                }
            }

            // BP failed - compute initial parity check violations
            let nharderrors = compute_nharderrors(llr);

            // Check if OSD is worth trying
            // Note: nharderrors here is parity check violations, not bit errors.
            // For weak signals with 15-20 bit errors, parity violations can exceed 40.
            if nharderrors > 50 {
                // Too many parity violations - OSD won't help
                return None;
            }

            // CRITICAL: Try OSD with BP-accumulated LLR snapshots (WSJT-X style)
            // This is the key to decoding difficult signals like N1PJT!
            // The accumulated LLRs have a smoothing effect that improves OSD performance.
            // Try ndeep=3 on all snapshots first (fast), then ndeep=4 on first snapshot only.
            for snapshot in &bp_snapshots {
                if let Some(decoded) = osd_decode_wsjt(snapshot, 3) {
                    return Some((decoded, 0, nharderrors));
                }
            }
            // If ndeep=3 failed, try ndeep=4 on first snapshot only (slower but thorough)
            if let Some(snapshot) = bp_snapshots.first() {
                if let Some(decoded) = osd_decode_wsjt(snapshot, 4) {
                    return Some((decoded, 0, nharderrors));
                }
            }

            // Fallback: try OSD order-1 with channel LLRs (very fast, 91 patterns)
            if let Some(decoded) = osd_decode(llr, 1) {
                return Some((decoded, 0, nharderrors));
            }

            None
        }
    }
}
