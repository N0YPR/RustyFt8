use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use bitvec::prelude::*;
use crate::message::CallsignHashCache;
use crate::message::callsign::unpack_callsign;
use crate::message::grid::decode_grid;
use crate::message::text_encoding::decode_callsign_base38;
use crate::message::constants::{NTOKENS, MAX22};

/// Decode Type 1 messages (i3=1)
pub fn decode_type1(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    // Check n3 subtype
    let n3: u8 = bits[3..6].load_be();
    
    if n3 == 4 {
        // NonStandardCall message (i3=1, n3=4)
        decode_type1_nonstandard(bits, cache)
    } else {
        // Standard Type 1 message
        decode_type1_standard(bits, cache)
    }
}

/// Decode Type 1 Standard message
fn decode_type1_standard(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    let mut bit_index = 0;
    
    // n28a: Decode first callsign (bits 0-27)
    let n28a: u32 = bits[bit_index..bit_index + 28].load_be();
    let call1 = if n28a >= NTOKENS && n28a < NTOKENS + MAX22 {
        // Hash callsign - look up in cache
        let ihash = n28a - NTOKENS;
        if let Some(cache_ref) = cache {
            if let Some(callsign) = cache_ref.lookup_22bit(ihash) {
                format!("<{}>", callsign)
            } else {
                format!("<...{:06X}>", ihash)
            }
        } else {
            format!("<...{:06X}>", ihash)
        }
    } else {
        unpack_callsign(n28a)?
    };
    bit_index += 28;
    
    // ipa: /P or /R suffix for first callsign (bit 28)
    let call1_suffix = bits[bit_index];
    bit_index += 1;
    
    // n28b: Decode second callsign (bits 29-56)
    let n28b: u32 = bits[bit_index..bit_index + 28].load_be();
    let mut call2 = if n28b >= NTOKENS && n28b < NTOKENS + MAX22 {
        // Hash callsign - look up in cache
        let ihash = n28b - NTOKENS;
        if let Some(cache_ref) = cache {
            if let Some(callsign) = cache_ref.lookup_22bit(ihash) {
                format!("<{}>", callsign)
            } else {
                format!("<...{:06X}>", ihash)
            }
        } else {
            format!("<...{:06X}>", ihash)
        }
    } else {
        unpack_callsign(n28b)?
    };
    bit_index += 28;
    
    // ipb: /P or /R suffix for second callsign (bit 57)
    let call2_suffix = bits[bit_index];
    bit_index += 1;
    
    // ir: R/acknowledge flag (bit 58)
    let r_flag = bits[bit_index];
    bit_index += 1;
    
    // igrid4: Decode grid square or report (bits 59-73)
    let grid_value: u16 = bits[bit_index..bit_index + 15].load_be();
    let mut grid_or_report = decode_grid(grid_value)?;
    
    // If r_flag is set, add "R" prefix
    // For signal reports (starts with + or -), it becomes "R+10" or "R-15"
    // For grids and special codes (RRR, RR73, 73), it becomes "R FN42" or "R RRR" (with space)
    if r_flag {
        if grid_or_report.starts_with('+') || grid_or_report.starts_with('-') {
            // Signal report: no space, e.g., "R+10" or "R-15"
            grid_or_report = format!("R{}", grid_or_report);
        } else {
            // Grid square or special code: with space, e.g., "R FN42" or "R RRR"
            grid_or_report = format!("R {}", grid_or_report);
        }
    }
    
    // Reconstruct the message text
    // Apply suffixes if present
    let mut final_call1 = call1;
    if call1_suffix {
        final_call1.push_str("/R");
    }
    
    if call2_suffix {
        call2.push_str("/R");
    }
    
    // Build the message string
    // If grid_or_report is empty (BLANK), don't include it
    if grid_or_report.is_empty() {
        Ok(format!("{} {}", final_call1, call2))
    } else {
        Ok(format!("{} {} {}", final_call1, call2, grid_or_report))
    }
}

/// Decode Type 1.4 NonStandardCall message
fn decode_type1_nonstandard(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    let mut bit_index = 0;
    
    // Skip i3 (3 bits)
    bit_index += 3;
    
    // Skip n3 (3 bits)
    bit_index += 3;
    
    // n12: 12-bit hash (bits 6-17)
    let _n12: u16 = bits[bit_index..bit_index + 12].load_be();
    bit_index += 12;
    
    // c58: Encoded text (bits 18-75, 58 bits)
    let c58: u64 = bits[bit_index..bit_index + 58].load_be();

    // Decode the text
    let text = decode_callsign_base38(c58)?;
    
    // Extract compound callsign and add to cache
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() >= 2 {
        let _compound_callsign = parts[1];
        if let Some(cache_ref) = cache {
            // Note: cache is immutable here, can't insert
            // The cache should have been populated during encoding
            let _ = cache_ref;  // Suppress unused warning
        }
    }
    
    Ok(text.trim_end().to_string())
}
