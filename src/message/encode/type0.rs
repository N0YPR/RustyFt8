use alloc::string::String;
use alloc::format;
use bitvec::prelude::*;
use crate::message::CallsignHashCache;
use crate::message::types::MessageVariant;
use crate::message::callsign::{pack_callsign, hash10};
use crate::message::text_encoding::encode_free_text;

/// Encode Type 0.0 Free Text message (i3=0, n3=0)
pub fn encode_free_text_msg(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>) -> Result<(), String> {
    if let MessageVariant::FreeText { text } = variant {
        let encoded = encode_free_text(text)?;
        
        // Copy the 71 bits of encoded text (bits 0-70)
        // The encoded array is 9 bytes, stored in format b7.7,8b8.8:
        // - First byte: bits 1-7 (7 bits, skip MSB)
        // - Remaining 8 bytes: all 64 bits
        // So we skip the first bit (bit 0 of byte 0) and copy bits 1-71
        let temp_bits = BitSlice::<u8, Msb0>::from_slice(&encoded);
        output[0..71].copy_from_bitslice(&temp_bits[1..72]);
        
        // n3: Subtype (3 bits, value = 0 for free text) - bits 71-73
        output[71..74].store_be(0u8);
        
        // i3: Message type (3 bits, value = 0 for Type 0) - bits 74-76
        output[74..77].store_be(0u8);
        
        Ok(())
    } else {
        Err("Expected FreeText variant".into())
    }
}

/// Encode Type 0.1 DXpedition message (i3=0, n3=1)
pub fn encode_dxpedition(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>, cache: Option<&mut CallsignHashCache>) -> Result<(), String> {
    if let MessageVariant::DXpedition { call1, call2, hash_call, report } = variant {
        let mut bit_index = 0;
        
        // n28a: Encode first callsign (28 bits)
        let n28a = pack_callsign(call1)?;
        output[bit_index..bit_index + 28].store_be(n28a);
        bit_index += 28;
        
        // n28b: Encode second callsign (28 bits)
        let n28b = pack_callsign(call2)?;
        output[bit_index..bit_index + 28].store_be(n28b);
        bit_index += 28;
        
        // n10: 10-bit hash of the callsign (10 bits)
        let n10 = hash10(hash_call) as u16;
        output[bit_index..bit_index + 10].store_be(n10);
        bit_index += 10;
        
        // Store in cache for later decoding
        if let Some(cache) = cache {
            cache.insert(hash_call);
        }
        
        // n5: Signal report encoded as (report+30)/2 (5 bits)
        // Range: -30 to +32 dB maps to 0-31
        let n5 = ((report + 30) / 2) as u8;
        output[bit_index..bit_index + 5].store_be(n5);
        bit_index += 5;
        
        // n3: Subtype (3 bits, value = 1 for DXpedition) - bits 71-73
        output[bit_index..bit_index + 3].store_be(1u8);
        bit_index += 3;
        
        // i3: Message type (3 bits, value = 0 for Type 0) - bits 74-76
        output[bit_index..bit_index + 3].store_be(0u8);
        
        Ok(())
    } else {
        Err("Expected DXpedition variant".into())
    }
}

/// Encode Type 0.3/0.4 ARRL Field Day message (i3=0, n3=3 or n3=4)
pub fn encode_field_day(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>) -> Result<(), String> {
    if let MessageVariant::FieldDay { call1, call2, r_flag, intx, nclass, isec, n3 } = variant {
        let mut bit_index = 0;
        
        // n28a: Encode first callsign (28 bits)
        let n28a = pack_callsign(call1)?;
        output[bit_index..bit_index + 28].store_be(n28a);
        bit_index += 28;
        
        // n28b: Encode second callsign (28 bits)
        let n28b = pack_callsign(call2)?;
        output[bit_index..bit_index + 28].store_be(n28b);
        bit_index += 28;
        
        // ir: R/acknowledge flag (1 bit)
        output.set(bit_index, *r_flag);
        bit_index += 1;
        
        // intx: Number of transmitters - 1 (or - 17 for n3=4) - 4 bits
        output[bit_index..bit_index + 4].store_be(*intx);
        bit_index += 4;
        
        // nclass: Class letter (0=A, 1=B, ..., 5=F) - 3 bits
        output[bit_index..bit_index + 3].store_be(*nclass);
        bit_index += 3;
        
        // isec: ARRL section code (1-86) - 7 bits
        output[bit_index..bit_index + 7].store_be(*isec);
        bit_index += 7;
        
        // n3: Subtype (3 or 4) - 3 bits
        output[bit_index..bit_index + 3].store_be(*n3);
        bit_index += 3;
        
        // i3: Message type (3 bits, value = 0 for Type 0) - 3 bits
        output[bit_index..bit_index + 3].store_be(0u8);
        
        Ok(())
    } else {
        Err("Expected FieldDay variant".into())
    }
}

/// Encode Type 0.5 Telemetry message (i3=0, n3=5)
pub fn encode_telemetry(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>) -> Result<(), String> {
    if let MessageVariant::Telemetry { hex_string } = variant {
        // Parse the 18 hex digits as 3 groups of 6
        let ntel1 = u32::from_str_radix(&hex_string[0..6], 16)
            .map_err(|_| format!("Invalid hex in telemetry: '{}'", hex_string))?;
        let ntel2 = u32::from_str_radix(&hex_string[6..12], 16)
            .map_err(|_| format!("Invalid hex in telemetry: '{}'", hex_string))?;
        let ntel3 = u32::from_str_radix(&hex_string[12..18], 16)
            .map_err(|_| format!("Invalid hex in telemetry: '{}'", hex_string))?;
        
        let mut bit_index = 0;
        
        // ntel1: First 6 hex digits (23 bits)
        output[bit_index..bit_index + 23].store_be(ntel1);
        bit_index += 23;
        
        // ntel2: Next 6 hex digits (24 bits)
        output[bit_index..bit_index + 24].store_be(ntel2);
        bit_index += 24;
        
        // ntel3: Last 6 hex digits (24 bits)
        output[bit_index..bit_index + 24].store_be(ntel3);
        bit_index += 24;
        
        // n3: Subtype (3 bits, value = 5 for Telemetry)
        output[bit_index..bit_index + 3].store_be(5u8);
        bit_index += 3;
        
        // i3: Message type (3 bits, value = 0 for Type 0)
        output[bit_index..bit_index + 3].store_be(0u8);
        
        Ok(())
    } else {
        Err("Expected Telemetry variant".into())
    }
}
