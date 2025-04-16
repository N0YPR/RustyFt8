use bitvec::prelude::*;

use std::collections::VecDeque;


use crate::util::bitvec_utils::{FromBitSlice, PackBitvecFieldType};
use super::{arrl_section::ArrlSection, callsign::Callsign, message_parse_error::MessageParseError, parse, serial_number_or_state_or_province::SerialNumberOrStateOrProvince, Message};

pub fn try_from_u128(message: u128) -> Result<Message, MessageParseError> {
    // https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
    // Type i3.n3 Purpose Example message Bit-fi eld tags
    // 0.5 Telemetry 123456789ABCDEF012 t71
    // t71 Telemetry data, up to 18 hexadecimal digits

    let mut message_bitvec: BitVec = BitVec::new();
    message.pack_into_bitvec(&mut message_bitvec, 77);
    let t71 = u128::from_bitslice(&message_bitvec[0..71]);
    let message_subtype = u8::from_bitslice(&message_bitvec[71..74]);
    let message_type = u8::from_bitslice(&message_bitvec[74..77]);

    // check message type
    if message_type != 0 || message_subtype != 5 {
        return Err(MessageParseError::InvalidMessage);
    }

    Ok(Message {
        message: message,
        display_string: format!("{:018X}", t71),
    })
}

pub fn try_from_string(value: &str) -> Result<Message, MessageParseError> {
    if value.len() > 18 {
        return Err(MessageParseError::InvalidMessage);
    }

    let value = match u128::from_str_radix(value, 16) {
        Ok(v) => v,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let mut message_bitvec: BitVec = BitVec::new();
    value.pack_into_bitvec(&mut message_bitvec, 71);
    5u8.pack_into_bitvec(&mut message_bitvec, 3);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = u128::from_bitslice(&message_bitvec[0..77]);

    Ok(Message {
        message,
        display_string: format!("{:018X}", value),
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_try_from_u128() {
        let message_bits = 0b00100100011010001010110011110001001101010111100110111101111000000010010101000;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "123456789ABCDEF012");
        assert_eq!(message.message, message_bits);
    }

    #[test]
    fn test_try_from_string() {
        let message_str = "123456789ABCDEF012";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "123456789ABCDEF012");
        assert_eq!(message.message, 0b00100100011010001010110011110001001101010111100110111101111000000010010101000);
    }
}