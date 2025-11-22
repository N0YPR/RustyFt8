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

pub use encode::encode;
pub use decode::{decode, decode_with_snapshots};
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
/// * `Some((message91, iterations))` - Decoded message and iteration count
/// * `None` - If all decode attempts failed
pub fn decode_hybrid(llr: &[f32], depth: DecodeDepth) -> Option<(BitVec<u8, Msb0>, usize)> {
    let max_bp_iters = 50; // Increased from 30 to give BP more chances to converge
    let osd_order = 2;

    match depth {
        DecodeDepth::BpOnly => {
            // BP only, no OSD fallback (fastest, fewest false positives)
            decode(llr, max_bp_iters)
        }

        DecodeDepth::BpOsdUncoupled => {
            // Try BP first (no snapshots needed)
            if let Some(result) = decode(llr, max_bp_iters) {
                return Some(result);
            }

            // BP failed, try OSD with channel LLRs only
            if let Some(decoded) = osd_decode(llr, osd_order) {
                return Some((decoded, 0)); // Return 0 to indicate OSD decode
            }

            None
        }

        DecodeDepth::BpOsdHybrid => {
            // Full hybrid strategy with BP snapshots
            let save_at_iters = [1, 2, 3];

            // Try BP first with snapshot saving
            match decode_with_snapshots(llr, max_bp_iters, &save_at_iters) {
                Ok((decoded, iters, _snapshots)) => {
                    // BP converged!
                    return Some((decoded, iters));
                }
                Err(snapshots) => {
                    // BP failed, try OSD with each saved snapshot
                    for (idx, snapshot_llr) in snapshots.iter().enumerate() {
                        if let Some(decoded) = osd_decode(snapshot_llr, osd_order) {
                            eprintln!("  OSD succeeded with iteration {} LLRs (order {})",
                                      save_at_iters[idx], osd_order);
                            return Some((decoded, 0));
                        }
                    }

                    // All snapshot attempts failed, fall back to channel LLRs
                    if let Some(decoded) = osd_decode(llr, osd_order) {
                        eprintln!("  OSD succeeded with channel LLRs (order {})", osd_order);
                        return Some((decoded, 0));
                    }
                }
            }

            None
        }
    }
}
