// Core modules
mod callsign;
mod grid;
mod callsign_cache;

// Refactored message77 modules
mod types;
mod lookup_tables;
mod validation;
mod text_encoding;
mod parser;
mod encode;
mod decode;

#[cfg(test)]
mod tests;

// Public API - main encoding/decoding functions
pub use encode::encode_variant;
pub use decode::decode_message_bits;
pub use parser::parse_message_variant;
pub use types::MessageVariant;

// Re-export callsign and grid utilities
pub use callsign::{pack_callsign, unpack_callsign, hash10, hash12, hash22};
pub use grid::{encode_grid, decode_grid};
pub use callsign_cache::CallsignHashCache;

// Re-export lookup tables for external use
pub use lookup_tables::{arrl_section_to_index, arrl_section_from_index, 
                         rtty_state_to_index, rtty_state_from_index};

use alloc::string::String;
use bitvec::prelude::*;

/// Encode a text message into a 77-bit FT8 message
///
/// This parses the text and encodes it into 77 bits.
/// The encoder determines the appropriate message type (i3.n3) based on what fits.
///
/// # Arguments
/// * `text` - The message text (e.g., "CQ N0YPR DM42")
/// * `output` - Mutable bit slice to write the 77 bits into (must be exactly 77 bits)
/// * `cache` - Mutable reference to a CallsignHashCache for non-standard callsigns
///
/// # Examples
///
/// ```no_run
/// use bitvec::prelude::*;
/// use rustyft8::message::{encode, CallsignHashCache};
///
/// let mut cache = CallsignHashCache::new();
/// let mut storage = bitarr![u8, Msb0; 0; 80];  // 10 bytes
/// encode("CQ N0YPR DM42", &mut storage[0..77], &mut cache)?;
///
/// // Non-standard callsigns are automatically cached
/// encode("K1ABC RR73; W9XYZ <KH1/KH7Z> -08", &mut storage[0..77], &mut cache)?;
/// # Ok::<(), String>(())
/// ```
pub fn encode(text: &str, output: &mut BitSlice<u8, Msb0>, cache: &mut CallsignHashCache) -> Result<(), String> {
    if output.len() != 77 {
        return Err(alloc::format!("Output buffer must be exactly 77 bits, got {}", output.len()));
    }

    // 1. Parse text into MessageVariant (internal detail)
    let variant = parse_message_variant(text)?;

    // 2. Encode variant into 77 bits
    encode_variant(&variant, output, Some(cache))?;

    Ok(())
}

/// Decode a 77-bit FT8 message back to text
///
/// This reverses the encoding process, extracting the message type and fields
/// from the bit array and reconstructing the original text.
///
/// Note: The decoded text may differ from the original input due to encoding
/// limitations. For example:
/// - "CQ PJ4/K1ABC FN42" → decodes as "CQ K1ABC FN42" (prefix stripped)
/// - "CQ PJ4/K1ABC" → decodes as "CQ PJ4/K1ABC" (uses Type 4 encoding)
///
/// # Arguments
/// * `bits` - The 77-bit message (must be exactly 77 bits)
/// * `cache` - Optional reference to a CallsignHashCache for resolving DXpedition mode hashes
///
/// # Examples
///
/// ```no_run
/// use bitvec::prelude::*;
/// use rustyft8::message::{encode, decode, CallsignHashCache};
///
/// let mut cache = CallsignHashCache::new();
/// let mut storage = bitarr![u8, Msb0; 0; 80];
/// encode("CQ N0YPR DM42", &mut storage[0..77], &mut cache)?;
/// let text = decode(&storage[0..77], None)?;
/// assert_eq!(text, "CQ N0YPR DM42");
/// # Ok::<(), String>(())
/// ```
pub fn decode(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    if bits.len() != 77 {
        return Err(alloc::format!("Input must be exactly 77 bits, got {}", bits.len()));
    }
    
    decode_message_bits(bits, cache)
}
