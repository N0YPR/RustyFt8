use alloc::string::{String, ToString};
use alloc::format;
use bitvec::prelude::*;
use crate::message::callsign::decode_callsign;
use crate::message::lookup_tables::rtty_state_from_index;

/// Decode Type 3 ARRL RTTY Roundup message (i3=3)
pub fn decode_type3(bits: &BitSlice<u8, Msb0>) -> Result<String, String> {
    let mut bit_index = 0;
    
    // tu: "TU;" prefix (bit 0)
    let tu = bits[bit_index];
    bit_index += 1;
    
    // n28a: Decode first callsign (bits 1-28)
    let n28a: u32 = bits[bit_index..bit_index + 28].load_be();
    let call1 = decode_callsign(n28a)?;
    bit_index += 28;
    
    // n28b: Decode second callsign (bits 29-56)
    let n28b: u32 = bits[bit_index..bit_index + 28].load_be();
    let call2 = decode_callsign(n28b)?;
    bit_index += 28;
    
    // r: R/acknowledge flag (bit 57)
    let r_flag = bits[bit_index];
    bit_index += 1;
    
    // rst: Signal report middle digit (bits 58-60, 3 bits)
    let rst: u8 = bits[bit_index..bit_index + 3].load_be();
    bit_index += 3;
    
    // nexch: Exchange value (bits 61-73, 13 bits)
    let nexch: u16 = bits[bit_index..bit_index + 13].load_be();
    
    // Decode exchange: if >= 8000, it's a state/province; else it's a serial number
    let exchange = if nexch >= 8000 && nexch <= 8171 {
        // State/province code
        rtty_state_from_index(nexch)
            .ok_or_else(|| format!("Invalid RTTY state index: {}", nexch))?
            .to_string()
    } else if nexch >= 1 && nexch <= 7999 {
        // Serial number
        format!("{:04}", nexch)
    } else {
        return Err(format!("Invalid RTTY exchange value: {}", nexch));
    };
    
    // Build the message string
    let mut msg = String::new();
    
    // Add TU prefix if present
    if tu {
        msg.push_str("TU; ");
    }
    
    // Add callsigns
    msg.push_str(&call1);
    msg.push(' ');
    msg.push_str(&call2);
    msg.push(' ');
    
    // Add R flag if present
    if r_flag {
        msg.push_str("R ");
    }
    
    // Add signal report (5X9 where X is the rst value)
    msg.push_str(&format!("5{}", rst + 2));  // rst is 0-7, representing 2-9
    msg.push('9');
    msg.push(' ');
    
    // Add exchange
    msg.push_str(&exchange);
    
    Ok(msg)
}
