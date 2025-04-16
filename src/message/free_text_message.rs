use bitvec::prelude::*;

use super::radix::{FromStrCustomRadix, ParseRadixStringError, ToStrCustomRadix};

use crate::{constants::FT8_CHAR_TABLE_FULL, util::bitvec_utils::{FromBitSlice, PackBitvecFieldType}};
use super::{message_parse_error::MessageParseError, Message};

pub fn try_from_u128(message: u128) -> Result<Message, MessageParseError> {
    // https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
    // Type i3.n3 Purpose Example message Bit-fi eld tags
    // 0.0 Free Text TNX BOB 73 GL f71
    // f71 Free text, up to 13 characters

    let mut message_bitvec: BitVec= BitVec::new();
    message.pack_into_bitvec(&mut message_bitvec, 77);
    let f71 = u128::from_bitslice(&message_bitvec[0..71]);
    let message_subtype = u8::from_bitslice(&message_bitvec[71..74]);
    let message_type = u8::from_bitslice(&message_bitvec[74..77]);

    // check message type
    if message_type != 0 || message_subtype != 0 {
        return Err(MessageParseError::InvalidMessage);
    }

    let display_string = match f71.to_str_custom_radix(FT8_CHAR_TABLE_FULL) {
        Ok(value) => value,
        Err(ParseRadixStringError::InvalidChar) => {
            return Err(MessageParseError::InvalidChar);
        }
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    Ok(
        Message {
            message,
            display_string
        }
    )
}

pub fn try_from_string(value: &str) -> Result<Message, MessageParseError> {
    // the string to pack into bits must be 13 characters
    //   right align with spaces if needed
    //   trim to length if needed
    let adjusted_string: String = format!("{: >13.13}", value);
    let packed_string = adjusted_string.trim().to_owned();

    // pack the string into a u128, 71 bits.
    let f71 = match u128::from_str_custom_radix(&adjusted_string, FT8_CHAR_TABLE_FULL) {
        Ok(value) => value,
        Err(ParseRadixStringError::InvalidChar) => {
            return Err(MessageParseError::InvalidChar);
        }
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    // pack all the bits together
    let mut message_bitvec: BitVec = BitVec::new();
    f71.pack_into_bitvec(&mut message_bitvec, 71);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = u128::from_bitslice(&message_bitvec[0..77]);

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_try_from_u128() {
        let message_bits = 0b01100011111011011100111011100010101001001010111000000111111101010000000000000;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "TNX BOB 73 GL");
        assert_eq!(message.message, message_bits);
    }

    #[test]
    fn test_try_from_string() {
        let message_str = "TNX BOB 73 GL";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "TNX BOB 73 GL");
        assert_eq!(message.message, 0b01100011111011011100111011100010101001001010111000000111111101010000000000000);
    }
}