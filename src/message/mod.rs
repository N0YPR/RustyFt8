use std::fmt::Display;

use message_parse_error::MessageParseError;

use crate::constants::{CHANNEL_SYMBOLS_COUNT, FT8_COSTAS};

mod arrl_section;
mod callsign;
mod channel_symbols;
mod checksum;
mod gray;
mod ldpc;
mod parse;
mod radix;
mod report;
mod serial_number_or_state_or_province;

#[cfg(test)]
mod tests;

pub mod message_parse_error;
pub mod try_from_str_for_message;
pub mod try_from_u8_slice_for_message;

#[derive(Debug)]
pub struct Message {
    pub message: u128,
    pub checksum: u16,
    pub parity: u128,
    pub channel_symbols: Vec<u8>,
    pub display_string: String,
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string)
    }
}
