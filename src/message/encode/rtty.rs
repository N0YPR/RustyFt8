use alloc::string::String;
use alloc::format;
use bitvec::prelude::*;
use crate::message::types::MessageVariant;
use crate::message::callsign::pack_callsign;
use crate::message::lookup_tables::rtty_state_to_index;

/// Encode Type 3 ARRL RTTY Roundup message (i3=3)
pub fn encode_rtty_roundup(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>) -> Result<(), String> {
    if let MessageVariant::RttyRoundup { tu, call1, call2, r_flag, rst, exchange } = variant {
        let mut bit_index = 0;
        
        // tu: "TU;" flag (1 bit)
        output.set(bit_index, *tu);
        bit_index += 1;
        
        // n28a: Encode first callsign (28 bits)
        let n28a = pack_callsign(call1)?;
        output[bit_index..bit_index + 28].store_be(n28a);
        bit_index += 28;
        
        // n28b: Encode second callsign (28 bits)
        let n28b = pack_callsign(call2)?;
        output[bit_index..bit_index + 28].store_be(n28b);
        bit_index += 28;
        
        // r: R/acknowledge flag (1 bit)
        output.set(bit_index, *r_flag);
        bit_index += 1;
        
        // rst: Signal report (3 bits) - middle digit of 5X9
        output[bit_index..bit_index + 3].store_be(*rst);
        bit_index += 3;
        
        // nexch: Exchange value (13 bits)
        // If numeric (serial), use value directly (1-7999)
        // If state/province, look up index and add 8000
        let nexch: u16 = if exchange.chars().all(|c| c.is_ascii_digit()) {
            exchange.parse::<u16>()
                .map_err(|_| format!("Invalid serial number: {}", exchange))?
        } else {
            // Look up state/province code
            rtty_state_to_index(exchange)
                .ok_or_else(|| format!("Unknown state/province: {}", exchange))?
        };
        output[bit_index..bit_index + 13].store_be(nexch);
        bit_index += 13;
        
        // i3: Message type (3 bits, value = 3 for RTTY Roundup)
        output[bit_index..bit_index + 3].store_be(3u8);
        
        Ok(())
    } else {
        Err("Expected RttyRoundup variant".into())
    }
}
