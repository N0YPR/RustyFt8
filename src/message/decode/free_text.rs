use bitvec::prelude::*;
use crate::message::CallsignHashCache;
use crate::message::callsign::unpack_callsign;
use crate::message::text_encoding::decode_free_text;
use crate::message::lookup_tables::arrl_section_from_index;

/// Decode Type 0 messages (i3=0)
pub fn decode_type0(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    // Check n3 subtype
    let n3: u8 = bits[71..74].load_be();
    
    match n3 {
        0 => decode_free_text_msg(bits),
        1 => decode_dxpedition(bits, cache),
        3 | 4 => decode_field_day(bits, n3),
        5 => decode_telemetry(bits),
        _ => Err(format!("Unsupported Type 0 subtype n3={}", n3))
    }
}

/// Decode Type 0.0 Free Text message
fn decode_free_text_msg(bits: &BitSlice<u8, Msb0>) -> Result<String, String> {
    // Extract the 71 bits of text data
    // The format is b7.7,8b8.8, so we need to insert a 0 bit at the start
    let mut text_bytes = [0u8; 9];
    let text_bits = BitSlice::<u8, Msb0>::from_slice_mut(&mut text_bytes);
    // Copy bits to positions 1-71 (skip bit 0 which stays 0)
    text_bits[1..72].copy_from_bitslice(&bits[0..71]);
    
    let text = decode_free_text(&text_bytes)?;
    Ok(text.trim_end().to_string())
}

/// Decode Type 0.1 DXpedition message
fn decode_dxpedition(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    let mut bit_index = 0;
    
    // n28a: Decode first callsign (bits 0-27)
    let n28a: u32 = bits[bit_index..bit_index + 28].load_be();
    let call1 = unpack_callsign(n28a)?;
    bit_index += 28;
    
    // n28b: Decode second callsign (bits 28-55)
    let n28b: u32 = bits[bit_index..bit_index + 28].load_be();
    let call2 = unpack_callsign(n28b)?;
    bit_index += 28;
    
    // n10: 10-bit hash (bits 56-65)
    let n10: u16 = bits[bit_index..bit_index + 10].load_be();
    bit_index += 10;
    
    // n5: Signal report (bits 66-70)
    let n5: u8 = bits[bit_index..bit_index + 5].load_be();
    let report = (n5 as i8) * 2 - 30;
    
    // Try to look up the callsign from the cache
    let hash_call = cache.and_then(|c| c.lookup_10bit(n10).map(|s| s.to_string()));
    
    // Format the hash display with angle brackets
    let hash_display = if let Some(call) = hash_call {
        format!("<{}>", call)
    } else {
        format!("<...{}>", n10)
    };
    
    let report_str = if report >= 0 {
        format!("+{:02}", report)
    } else {
        format!("{:03}", report)  // Use 03 for negative numbers to get -08 format
    };
    
    Ok(format!("{} RR73; {} {} {}", call1, call2, hash_display, report_str))
}

/// Decode Type 0.3/0.4 ARRL Field Day message
fn decode_field_day(bits: &BitSlice<u8, Msb0>, n3: u8) -> Result<String, String> {
    let mut bit_index = 0;
    
    // n28a: Decode first callsign (bits 0-27)
    let n28a: u32 = bits[bit_index..bit_index + 28].load_be();
    let call1 = unpack_callsign(n28a)?;
    bit_index += 28;
    
    // n28b: Decode second callsign (bits 28-55)
    let n28b: u32 = bits[bit_index..bit_index + 28].load_be();
    let call2 = unpack_callsign(n28b)?;
    bit_index += 28;
    
    // ir: R/acknowledge flag (bit 56)
    let r_flag = bits[bit_index];
    bit_index += 1;
    
    // intx: Number of transmitters - 1 (or - 17 for n3=4) - bits 57-60
    let intx: u8 = bits[bit_index..bit_index + 4].load_be();
    bit_index += 4;
    
    // nclass: Class letter (bits 61-63)
    let nclass: u8 = bits[bit_index..bit_index + 3].load_be();
    bit_index += 3;
    
    // isec: ARRL section code (bits 64-70)
    let isec: u8 = bits[bit_index..bit_index + 7].load_be();
    
    // Calculate actual transmitter count
    let ntx = if n3 == 3 {
        intx + 1
    } else {
        intx + 17
    };
    
    // Convert class code to letter (0=A, 1=B, etc.)
    let class_letter = (b'A' + nclass) as char;
    
    // Look up section abbreviation
    let section = arrl_section_from_index(isec)
        .ok_or_else(|| format!("Invalid ARRL section code: {}", isec))?;
    
    // Format the output with proper spacing based on WSJT-X rules
    if r_flag {
        if ntx < 10 {
            Ok(format!("{} {} R{}{} {}", call1, call2, ntx, class_letter, section))
        } else {
            Ok(format!("{} {} R {}{} {}", call1, call2, ntx, class_letter, section))
        }
    } else {
        if ntx < 10 {
            Ok(format!("{} {} {}{} {}", call1, call2, ntx, class_letter, section))
        } else {
            Ok(format!("{} {} {}{} {}", call1, call2, ntx, class_letter, section))
        }
    }
}

/// Decode Type 0.5 Telemetry message
fn decode_telemetry(bits: &BitSlice<u8, Msb0>) -> Result<String, String> {
    let mut bit_index = 0;
    
    // ntel1: First 6 hex digits (bits 0-22, 23 bits)
    let ntel1: u32 = bits[bit_index..bit_index + 23].load_be();
    bit_index += 23;
    
    // ntel2: Next 6 hex digits (bits 23-46, 24 bits)
    let ntel2: u32 = bits[bit_index..bit_index + 24].load_be();
    bit_index += 24;
    
    // ntel3: Last 6 hex digits (bits 47-70, 24 bits)
    let ntel3: u32 = bits[bit_index..bit_index + 24].load_be();
    
    // Format as 18 hex digits (3 groups of 6)
    let hex_string = format!("{:06X}{:06X}{:06X}", ntel1, ntel2, ntel3);
    
    // Strip leading zeros like WSJT-X does
    let trimmed = hex_string.trim_start_matches('0');
    if trimmed.is_empty() {
        Ok("0".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}
