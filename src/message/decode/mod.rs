use alloc::string::String;
use alloc::format;
use bitvec::prelude::*;
use crate::message::CallsignHashCache;

mod free_text;
mod standard;
mod eu_vhf;
mod rtty_field_day;
mod nonstandard;

pub use free_text::*;
pub use standard::*;
pub use eu_vhf::*;
pub use rtty_field_day::*;
pub use nonstandard::*;

/// Decode 77-bit message back to text
pub fn decode_message_bits(bits: &BitSlice<u8, Msb0>, cache: Option<&CallsignHashCache>) -> Result<String, String> {
    // Extract i3 (message type) from bits 74-76
    let i3: u8 = bits[74..77].load_be();
    
    match i3 {
        0 => decode_type0(bits, cache),
        1 => decode_type1(bits, cache),
        2 => decode_type2(bits, cache),
        3 => decode_type3(bits),
        4 => decode_type4(bits, cache),
        _ => Err(format!("Unsupported message type i3={}", i3))
    }
}
