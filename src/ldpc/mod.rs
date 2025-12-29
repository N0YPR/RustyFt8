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
mod decode_bp;
mod decode_hybrid;
mod decode_osd;
mod platanh;

use bitvec::prelude::*;

pub use encode::encode;
pub use decode_bp::MAX_BP_ITERATIONS;
pub use decode_osd::{osd_decode, osd_decode_wsjt};

/// Decoding depth strategy (matches WSJT-X ndepth setting)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeDepth {
    /// ndepth=1, maxosd=-1: BP only, fastest, fewest false positives
    Fast,
    /// ndepth=2, maxosd=0: BP + single OSD pass with channel LLRs
    Normal,
    /// ndepth=3, maxosd=2: BP + multiple OSD iterations with BP snapshots
    Deep,
}

/// Decode a 174-bit codeword using LDPC with configurable depth
///
/// This implements WSJT-X's decode174_91.f90 approach with configurable depth:
///
/// **Fast** (ndepth=1): BP only, fastest, fewest false positives
/// **Normal** (ndepth=2): BP + OSD order-2 fallback
/// **Deep** (ndepth=3): BP with snapshots + OSD with accumulated LLRs
///
/// # Arguments
/// * `llr` - Log-Likelihood Ratios for 174 bits
/// * `apmask` - Optional AP mask; bits marked `true` stay fixed at their LLR values
/// * `depth` - Decoding depth strategy
///
/// # Returns
/// * `Some((message91, iterations, nharderrors))` on success
/// * `None` if all decode attempts failed
pub fn decode(
    llr: &[f32],
    apmask: Option<&[bool]>,
    depth: DecodeDepth,
) -> Option<(BitVec<u8, Msb0>, usize, usize)> {
    match depth {
        DecodeDepth::Fast => {
            // BP only, no OSD fallback
            decode_bp::decode(llr, apmask)
        }

        DecodeDepth::Normal => {
            // Try BP first
            if let Some(result) = decode_bp::decode(llr, apmask) {
                return Some(result);
            }

            // BP failed - check if OSD is worth trying
            let nharderrors = decode_bp::compute_nharderrors(llr);
            if nharderrors > 50 {
                return None;
            }

            // Try OSD order-2 (C(91,2) = 4095 patterns)
            if let Some(decoded) = osd_decode(llr, 2) {
                return Some((decoded, 0, nharderrors));
            }

            None
        }

        DecodeDepth::Deep => {
            // Save accumulated LLR snapshots at iterations 1, 2, 3 (matching WSJT-X)
            let save_at_iters = [1, 2, 3];
            let bp_snapshots: Vec<Vec<f32>>;

            if apmask.is_some() {
                // With AP mask, use regular BP (no snapshots)
                if let Some(result) = decode_bp::decode(llr, apmask) {
                    return Some(result);
                }
                bp_snapshots = Vec::new();
            } else {
                // Try BP with snapshots
                match decode_hybrid::decode_with_snapshots(llr, &save_at_iters) {
                    Ok((decoded, iters, nharderrors, _)) => {
                        return Some((decoded, iters, nharderrors));
                    }
                    Err(snapshots) => {
                        bp_snapshots = snapshots;
                    }
                }
            }

            // BP failed - compute initial parity check violations
            let nharderrors = decode_bp::compute_nharderrors(llr);
            if nharderrors > 50 {
                return None;
            }

            // Try OSD with BP-accumulated LLR snapshots (WSJT-X style)
            for snapshot in &bp_snapshots {
                if let Some(decoded) = osd_decode_wsjt(snapshot, 3) {
                    return Some((decoded, 0, nharderrors));
                }
            }

            // Fallback: try OSD order-1 with channel LLRs
            if let Some(decoded) = osd_decode(llr, 1) {
                return Some((decoded, 0, nharderrors));
            }

            None
        }
    }
}
