use bitvec::prelude::*;
use crate::message::CallsignHashCache;
use crate::message::constants::CHARSET_BASE38;

/// Decode Type 4 NonStandardCall message (i3=4)
pub fn decode_type4(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    let mut bit_index = 0;

    // n12: 12-bit hash (bits 0-11)
    let n12: u16 = bits[bit_index..bit_index + 12].load_be();
    bit_index += 12;

    // n58: base-38 encoded callsign (bits 12-69)
    let n58: u64 = bits[bit_index..bit_index + 58].load_be();
    bit_index += 58;

    // Decode n58 as base-38 callsign
    let mut acc = n58;
    let mut callsign = String::with_capacity(11);
    for _ in 0..11 {
        let idx = (acc % 38) as usize;
        callsign.push(CHARSET_BASE38[idx] as char);
        acc /= 38;
    }
    callsign = callsign.chars().rev().collect::<String>().trim_start().to_string();
    
    // iflip: position flag (bit 70)
    let iflip = bits[bit_index];
    bit_index += 1;
    
    // nrpt: report/ack type (bits 71-72)
    let nrpt: u8 = bits[bit_index..bit_index + 2].load_be();
    bit_index += 2;
    
    // icq: CQ flag (bit 73)
    let icq = bits[bit_index];
    // bit_index += 1;  // Last field, no need to advance
    
    // Build the message based on iflip and icq
    let mut msg = if icq {
        // CQ message: "CQ COMPOUND" (iflip=0)
        format!("CQ {}", callsign)
    } else {
        // Non-CQ message with hash callsign
        // Try to look up the hash callsign from cache
        let hash_call = if let Some(cache_ref) = cache {
            cache_ref.lookup_12bit(n12)
                .map(|s| format!("<{}>", s))
                .unwrap_or_else(|| format!("<...>"))
        } else {
            format!("<...>")
        };
        
        if iflip {
            // iflip=1: "COMPOUND <HASH>"
            format!("{} {}", callsign, hash_call)
        } else {
            // iflip=0: "<HASH> COMPOUND"
            format!("{} {}", hash_call, callsign)
        }
    };
    
    // Add acknowledgment/report if present
    match nrpt {
        1 => msg.push_str(" RRR"),
        2 => msg.push_str(" RR73"),
        3 => msg.push_str(" 73"),
        _ => {}  // 0 = no acknowledgment
    }
    
    Ok(msg)
}
