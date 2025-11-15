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

pub use encode::encode;
pub use decode::decode;
