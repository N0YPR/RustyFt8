use alloc::string::String;
use bitvec::prelude::*;
use crate::message::CallsignHashCache;
use crate::message::types::MessageVariant;

mod standard;
mod rtty;
mod type0;
mod nonstandard;

pub use standard::*;
pub use rtty::*;
pub use type0::*;
pub use nonstandard::*;

/// Encode a MessageVariant into 77 bits
pub fn encode_variant(variant: &MessageVariant, output: &mut BitSlice<u8, Msb0>, cache: Option<&mut CallsignHashCache>) -> Result<(), String> {
    match variant {
        MessageVariant::Standard { .. } => encode_standard(variant, output, cache),
        MessageVariant::EuVhfContestType2 { .. } => encode_type2(variant, output, cache),
        MessageVariant::RttyRoundup { .. } => encode_rtty_roundup(variant, output),
        MessageVariant::FreeText { .. } => encode_free_text_msg(variant, output),
        MessageVariant::DXpedition { .. } => encode_dxpedition(variant, output, cache),
        MessageVariant::FieldDay { .. } => encode_field_day(variant, output),
        MessageVariant::Telemetry { .. } => encode_telemetry(variant, output),
        MessageVariant::NonStandardCall { .. } => encode_nonstandard_call(variant, output, cache),
    }
}
