use alloc::string::String;
use bitvec::prelude::*;
use crate::message::CallsignHashCache;
use crate::message::types::MessageVariant;
use crate::message::callsign::{encode_callsign, ihashcall};
use crate::message::grid::encode_grid;

const NTOKENS: u32 = 2063592;

/// Encode Type 1 Standard message (i3=1)
pub fn encode_standard(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>, mut cache: Option<&mut CallsignHashCache>) -> Result<(), String> {
    if let MessageVariant::Standard { call1, call1_suffix, call2, call2_suffix, r_flag, grid_or_report } = variant {
        let mut bit_index = 0;
        
        // n28a: Encode first callsign (28 bits) - can be "CQ", "CQ SOTA", or regular callsign
        // Check if call1 is a hash callsign (starts with '<' and ends with '>')
        let n28a_value = if call1.starts_with('<') && call1.ends_with('>') {
            // Extract callsign from angle brackets
            let inner_call = &call1[1..call1.len()-1];
            let hash22 = ihashcall(inner_call, 22);
            let n28 = NTOKENS + hash22;
            
            // Add to cache if available
            if let Some(ref mut cache_mut) = cache {
                cache_mut.insert(inner_call);
            }
            
            n28
        } else {
            encode_callsign(call1)?
        };
        output[bit_index..bit_index + 28].store_be(n28a_value);
        bit_index += 28;
        
        // ipa: /R suffix for first call (1 bit) - Type 1 uses /R
        output.set(bit_index, *call1_suffix);
        bit_index += 1;
        
        // n28b: Encode second callsign to 28 bits
        // Check if call2 is a hash callsign (starts with '<' and ends with '>')
        let n28b_value = if call2.starts_with('<') && call2.ends_with('>') {
            // Extract callsign from angle brackets
            let inner_call = &call2[1..call2.len()-1];
            let hash22 = ihashcall(inner_call, 22);
            let n28 = NTOKENS + hash22;
            
            // Add to cache if available
            if let Some(ref mut cache_mut) = cache {
                cache_mut.insert(inner_call);
            }
            
            n28
        } else {
            encode_callsign(call2)?
        };
        output[bit_index..bit_index + 28].store_be(n28b_value);
        bit_index += 28;
        
        // ipb: /R suffix for second call (1 bit) - Type 1 uses /R
        output.set(bit_index, *call2_suffix);
        bit_index += 1;
        
        // ir: R/acknowledge flag (1 bit)
        output.set(bit_index, *r_flag);
        bit_index += 1;
        
        // igrid4: Encode grid square or report (15 bits)
        let grid_value = encode_grid(grid_or_report)?;
        output[bit_index..bit_index + 15].store_be(grid_value);
        bit_index += 15;
        
        // i3: Message type (3 bits, value = 1 for Type 1)
        output[bit_index..bit_index + 3].store_be(1u8);
        
        Ok(())
    } else {
        Err("Expected Standard variant".into())
    }
}

/// Encode Type 2 EU VHF Contest message (i3=2) - compound callsigns with /P suffix
pub fn encode_type2(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>, mut cache: Option<&mut CallsignHashCache>) -> Result<(), String> {
    if let MessageVariant::EuVhfContestType2 { call1, call1_suffix, call2, call2_suffix, r_flag, grid_or_report } = variant {
        let mut bit_index = 0;
        
        // n28a: Encode first callsign (28 bits)
        let n28a_value = if call1.starts_with('<') && call1.ends_with('>') {
            let inner_call = &call1[1..call1.len()-1];
            let hash22 = ihashcall(inner_call, 22);
            let n28 = NTOKENS + hash22;
            
            if let Some(ref mut cache_mut) = cache {
                cache_mut.insert(inner_call);
            }
            
            n28
        } else {
            encode_callsign(call1)?
        };
        output[bit_index..bit_index + 28].store_be(n28a_value);
        bit_index += 28;
        
        // ipa: /P suffix for first call (1 bit) - Type 2 uses /P
        output.set(bit_index, *call1_suffix);
        bit_index += 1;
        
        // n28b: Encode second callsign to 28 bits
        let n28b_value = if call2.starts_with('<') && call2.ends_with('>') {
            let inner_call = &call2[1..call2.len()-1];
            let hash22 = ihashcall(inner_call, 22);
            let n28 = NTOKENS + hash22;
            
            if let Some(ref mut cache_mut) = cache {
                cache_mut.insert(inner_call);
            }
            
            n28
        } else {
            encode_callsign(call2)?
        };
        output[bit_index..bit_index + 28].store_be(n28b_value);
        bit_index += 28;
        
        // ipb: /P suffix for second call (1 bit) - Type 2 uses /P
        output.set(bit_index, *call2_suffix);
        bit_index += 1;
        
        // ir: R/acknowledge flag (1 bit)
        output.set(bit_index, *r_flag);
        bit_index += 1;
        
        // igrid4: Encode grid square or report (15 bits)
        let grid_value = encode_grid(grid_or_report)?;
        output[bit_index..bit_index + 15].store_be(grid_value);
        bit_index += 15;
        
        // i3: Message type (3 bits, value = 2 for Type 2)
        output[bit_index..bit_index + 3].store_be(2u8);
        
        Ok(())
    } else {
        Err("Expected EuVhfContestType2 variant".into())
    }
}
