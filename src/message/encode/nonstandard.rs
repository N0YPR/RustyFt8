use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use bitvec::prelude::*;
use crate::message::CallsignHashCache;
use crate::message::types::MessageVariant;
use crate::message::callsign::ihashcall;

/// Encode Type 4 NonStandardCall message (i3=4)
pub fn encode_nonstandard_call(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>, mut cache: Option<&mut CallsignHashCache>) -> Result<(), String> {
    if let MessageVariant::NonStandardCall { text } = variant {
        // Parse the message
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(format!("NonStandardCall requires at least 2 words: '{}'", text));
        }
        
        let is_cq = parts[0].eq_ignore_ascii_case("CQ");
        let first_is_hash = parts[0].starts_with('<') && parts[0].ends_with('>');
        let second_is_hash = parts[1].starts_with('<') && parts[1].ends_with('>');
        
        // Parse acknowledgment/report if present (3rd word)
        let nrpt = if parts.len() >= 3 {
            match parts[2] {
                "RRR" => 1u8,
                "RR73" => 2u8,
                "73" => 3u8,
                _ => return Err(format!("Invalid acknowledgment in NonStandardCall: '{}'", parts[2])),
            }
        } else {
            0u8  // No acknowledgment
        };
        
        // Determine which callsign is compound and which is hash (or CQ)
        let (compound_callsign, hash_callsign, iflip, icq) = if is_cq {
            // Case 1: "CQ COMPOUND" (iflip=0, icq=1)
            (parts[1], None, false, true)
        } else if second_is_hash {
            // Case 2: "COMPOUND <HASH>" (iflip=1, icq=0)
            let hash_call = parts[1].trim_start_matches('<').trim_end_matches('>');
            (parts[0], Some(hash_call), true, false)
        } else if first_is_hash {
            // Case 3: "<HASH> COMPOUND" (iflip=0, icq=0)
            let hash_call = parts[0].trim_start_matches('<').trim_end_matches('>');
            (parts[1], Some(hash_call), false, false)
        } else {
            return Err(format!("NonStandardCall requires CQ or hash callsign: '{}'", text));
        };
        
        // n12: 12-bit hash
        // For CQ messages: hash of the compound callsign (for cache/lookup)
        // For non-CQ messages: hash of the explicit hash callsign
        let n12 = if icq {
            ihashcall(compound_callsign, 12) as u16
        } else if let Some(hash_call) = hash_callsign {
            ihashcall(hash_call, 12) as u16
        } else {
            0
        };
        
        // n58: base-38 encoding of the compound callsign (right-aligned to 11 chars)
        const CHARSET: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/";
        let padded = format!("{:>11}", compound_callsign.to_uppercase());
        let mut n58: u64 = 0;
        for ch in padded.bytes() {
            let idx = CHARSET.iter().position(|&c| c == ch)
                .ok_or_else(|| format!("Invalid character in callsign: '{}'", ch as char))?;
            n58 = n58 * 38 + idx as u64;
        }
        
        // Add callsigns to cache if available
        if let Some(ref mut cache_mut) = cache {
            // Always add the compound callsign
            cache_mut.insert(compound_callsign);
            // Also add the hash callsign if present (for non-CQ messages)
            if let Some(hash_call) = hash_callsign {
                cache_mut.insert(hash_call);
            }
        }
        
        let mut bit_index = 0;
        
        // n12: 12-bit hash (bits 0-11)
        output[bit_index..bit_index + 12].store_be(n12);
        bit_index += 12;
        
        // n58: base-38 encoded callsign (bits 12-69)
        output[bit_index..bit_index + 58].store_be(n58);
        bit_index += 58;
        
        // iflip: position flag (bit 70)
        output.set(bit_index, iflip);
        bit_index += 1;
        
        // nrpt: report/ack type (bits 71-72)
        // 0=none, 1=RRR, 2=RR73, 3=73
        output[bit_index..bit_index + 2].store_be(nrpt);
        bit_index += 2;
        
        // icq: CQ flag (bit 73)
        output.set(bit_index, icq);
        bit_index += 1;
        
        // i3: Message type (bits 74-76) - 4 for NonStandardCall
        output[bit_index..bit_index + 3].store_be(4u8);
        
        Ok(())
    } else {
        Err("Expected NonStandardCall variant".into())
    }
}
