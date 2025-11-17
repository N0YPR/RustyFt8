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

/// Hybrid BP/OSD decoder matching WSJT-X's strategy
///
/// This implements WSJT-X's decode174_91.f90 approach:
/// 1. Run BP for 30 iterations, saving LLR snapshots at iterations 1, 2, 3
/// 2. If BP converges, return immediately
/// 3. If BP fails, try OSD order 2 with each saved snapshot
///
/// This strategy explores different regions of the solution space:
/// - Iteration 1: Close to channel LLRs, less correlated errors
/// - Iteration 2: Partially converged, different error pattern
/// - Iteration 3: Further converged
///
/// WSJT-X parameters:
/// - maxosd=2: Try OSD 2 times (with BP snapshots from iters 1, 2)
/// - norder=2: OSD order 2
/// - max_bp_iters=30: BP iterations (vs our previous 200)
///
/// # Arguments
/// * `llr` - Log-Likelihood Ratios for 174 bits
///
/// # Returns
/// * `Some((message91, iterations))` - Decoded message and iteration count
/// * `None` - If all decode attempts failed
pub fn decode_hybrid(llr: &[f32]) -> Option<(BitVec<u8, Msb0>, usize)> {
    // WSJT-X parameters: 30 BP iterations, save at iters 1, 2, 3
    let max_bp_iters = 30;
    let save_at_iters = [1, 2, 3];

    // Try BP first with snapshot saving
    match decode_with_snapshots(llr, max_bp_iters, &save_at_iters) {
        Ok((decoded, iters, _snapshots)) => {
            // BP converged!
            return Some((decoded, iters));
        }
        Err(snapshots) => {
            // BP failed, try OSD with each saved snapshot
            // WSJT-X uses OSD order 2 (not 4)
            let osd_order = 2;

            for (idx, snapshot_llr) in snapshots.iter().enumerate() {
                if let Some(decoded) = osd_decode(snapshot_llr, osd_order) {
                    eprintln!("  OSD succeeded with iteration {} LLRs (order {})",
                              save_at_iters[idx], osd_order);
                    return Some((decoded, 0)); // Return 0 to indicate OSD decode
                }
            }

            // All attempts failed, fall back to trying OSD with channel LLRs
            // (WSJT-X's maxosd=0 mode)
            if let Some(decoded) = osd_decode(llr, osd_order) {
                eprintln!("  OSD succeeded with channel LLRs (order {})", osd_order);
                return Some((decoded, 0));
            }
        }
    }

    None
}
