use bitvec::prelude::*;

use std::collections::VecDeque;

use crate::util::bitvec_utils::{FromBitSlice, PackBitvecFieldType};
use super::{arrl_section::ArrlSection, callsign::Callsign, message_parse_error::MessageParseError, parse, Message};

pub fn try_from_u128(message: u128) -> Result<Message, MessageParseError> {
    // https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
    // Type i3.n3 Purpose Example message Bit-fi eld tags
    // 0.3 Field Day K1ABC W9XYZ 6A WI c28 c28 R1 n4 k3 S7
    // 0.4 Field Day W9XYZ K1ABC R 17B EMA c28 c28 R1 n4 k3 S7
    // c28 c28 R1 n4 k3 S7
    // c28 Standard callsign, CQ, DE, QRZ, or 22-bit hash
    // R1 R
    // n4 Number of transmitters: 1-16, 17-32
    // k3 Field Day Class: A, B, ... F
    // S7 ARRL/RAC Section

    let mut message_bitvec: BitVec = BitVec::new();
    message.pack_into_bitvec(&mut message_bitvec, 77);

    let c28_1 = u32::from_bitslice(&message_bitvec[0..28]);
    let c28_2 = u32::from_bitslice(&message_bitvec[28..56]);
    let r1 = message_bitvec[56];
    let n4 = u8::from_bitslice(&message_bitvec[57..61]);
    let k3 = u8::from_bitslice(&message_bitvec[61..64]);
    let s7 = u8::from_bitslice(&message_bitvec[64..71]);
    let message_subtype = u8::from_bitslice(&message_bitvec[71..74]);
    let message_type = u8::from_bitslice(&message_bitvec[74..77]);

    // check message type
    if message_type != 0 || (message_subtype != 3 && message_subtype != 4) {
        return Err(MessageParseError::InvalidMessage);
    }

    let callsign1 = match Callsign::try_from(c28_1) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let callsign2 = match Callsign::try_from(c28_2) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let ack = r1;

    let section = match ArrlSection::from_packed_bits(s7.into()) {
        Ok(s) => s,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let mut number_transmitters:u8 = n4 + 1;
    if message_subtype == 4 {
        // 4 is 16-32
        number_transmitters += 16;
    }
    let number_transmitters_string = format!("{}", number_transmitters);

    let field_day_class_string = match k3 {
        0 => "A",
        1 => "B",
        2 => "C",
        3 => "D",
        4 => "E",
        5 => "F",
        _ => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    
    let ack_string = if ack { "R " } else { "" };
    let packed_string = format!("{callsign1} {callsign2} {ack_string}{number_transmitters_string}{field_day_class_string} {section}");

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

pub fn try_from_string(value: &str) -> Result<Message, MessageParseError> {
    // parse into words
    let message_words = value.split_whitespace().collect::<Vec<&str>>();
    let mut deq = VecDeque::from_iter(message_words.iter().copied());

    let callsign1: Callsign = match parse::try_parse_callsign(&mut deq) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let callsign2: Callsign = match parse::try_parse_callsign(&mut deq) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let ack: bool = match parse::try_parse_ack(&mut deq) {
        Ok(b) => b,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let (
        number_transmitters,
        number_transmitters_string,
        sub_type,
        field_day_class,
        field_day_class_string,
    ) = match try_parse_transmitters_and_class(&mut deq) {
        Ok((n, ns, s, f, fs)) => (n, ns, s, f, fs),
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let section = match try_parse_arrl_section(&mut deq) {
        Ok(s) => s,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    // c28 c28 R1 n4 k3 S7
    // c28 Standard callsign, CQ, DE, QRZ, or 22-bit hash
    // R1 R
    // n4 Number of transmitters: 1-16, 17-32
    // k3 Field Day Class: A, B, ... F
    // S7 ARRL/RAC Section
    let mut message_bitvec: BitVec = BitVec::new();
    callsign1
        .packed_28bits
        .pack_into_bitvec(&mut message_bitvec, 28);
    callsign2
        .packed_28bits
        .pack_into_bitvec(&mut message_bitvec, 28);
    ack.pack_into_bitvec(&mut message_bitvec, 1);
    number_transmitters.pack_into_bitvec(&mut message_bitvec, 4);
    field_day_class.pack_into_bitvec(&mut message_bitvec, 3);
    section.packed_bits.pack_into_bitvec(&mut message_bitvec, 7);
    sub_type.pack_into_bitvec(&mut message_bitvec, 3);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = u128::from_bitslice(&message_bitvec[0..77]);

    // Field Day    W9XYZ K1ABC R 17B EMA   c28 c28 R1 n4 k3 S7
    let ack_string = if ack { "R " } else { "" };
    let packed_string = format!("{callsign1} {callsign2} {ack_string}{number_transmitters_string}{field_day_class_string} {section}");

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

fn try_parse_transmitters_and_class(
    deq: &mut VecDeque<&str>,
) -> Result<(u8, String, u8, u8, String), MessageParseError> {
    if let Some(word) = deq.pop_front() {
        let num_transmitters_string = all_except_last(word).to_string();
        let (num_transmitters, sub_type) =
            match u8::from_str_radix(&num_transmitters_string, 10) {
                Ok(value) => {
                    if value < 1 || value > 32 {
                        return Err(MessageParseError::InvalidMessage);
                    }
                    if value <= 16 {
                        (value - 1, 3u8)
                    } else {
                        (value - 16 - 1, 4u8)
                    }
                }
                Err(_) => {
                    return Err(MessageParseError::InvalidMessage);
                }
            };

        const ARRL_CLASSES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
        let field_day_class_string = last(word).to_string();
        let field_day_class: u8 = match ARRL_CLASSES
            .iter()
            .position(|s| s == &field_day_class_string)
        {
            Some(value) => value as u8,
            None => {
                return Err(MessageParseError::InvalidMessage);
            }
        };

        return Ok((
            num_transmitters,
            num_transmitters_string,
            sub_type,
            field_day_class,
            field_day_class_string,
        ));
    }
    return Err(MessageParseError::InvalidMessage);
}

fn all_except_last(value: &str) -> &str {
    let mut chars = value.chars();
    chars.next_back();
    chars.as_str()
}

fn last(value: &str) -> &str {
    &value[value.len() - 1..]
}

fn try_parse_arrl_section(deq: &mut VecDeque<&str>) -> Result<ArrlSection, MessageParseError> {
    if let Some(word) = deq.pop_front() {
        return match ArrlSection::try_from_string(word) {
            Ok(s) => Ok(s),
            Err(_) => Err(MessageParseError::InvalidMessage),
        };
    }
    return Err(MessageParseError::InvalidMessage);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_from_u128() {
        let message_bits = 0b00001001101111011110001101010000110000101001001110111000001010001001100011000;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "K1ABC W9XYZ 6A WI");
        assert_eq!(message.message, message_bits);
    }

    #[test]
    fn test_try_from_string() {
        let message_str = "K1ABC W9XYZ 6A WI";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "K1ABC W9XYZ 6A WI");
        assert_eq!(message.message, 0b00001001101111011110001101010000110000101001001110111000001010001001100011000);
    }

    #[test]
    fn test_try_from_string2() {
        let message_str = "W9XYZ K1ABC R 17B EMA";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "W9XYZ K1ABC R 17B EMA");
        assert_eq!(message.message, 0b00001100001010010011101110000000100110111101111000110101100000010001011100000);
    }

    #[test]
    fn test_try_from_u128_2() {
        let message_bits = 0b00001100001010010011101110000000100110111101111000110101100000010001011100000;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "W9XYZ K1ABC R 17B EMA");
        assert_eq!(message.message, message_bits);
    }
}
