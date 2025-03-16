use bitvec::prelude::*;

use std::collections::VecDeque;


use crate::util::bitvec_utils::{FromBitSlice, PackBitvecFieldType};
use super::{callsign::Callsign, message_parse_error::MessageParseError, parse, serial_number_or_state_or_province::SerialNumberOrStateOrProvince, Message};

pub fn try_from_u128(message: u128) -> Result<Message, MessageParseError> {
    // https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
    // Type i3.n3 Purpose Example message Bit-fi eld tags
    // 3. RTTY RU K1ABC W9XYZ 579 WI t1 c28 c28 R1 r3 s13
    // t1 TU;
    // c28 Standard callsign, CQ, DE, QRZ, or 22-bit hash
    // c28 Standard callsign, CQ, DE, QRZ, or 22-bit hash
    // R1 R
    // r3 Report: 2-9, displayed as 529 – 599 or 52 - 59
    // s13 Serial Number (0-7999) or State/Province
    let mut message_bitvec: BitVec = BitVec::new();
    message.pack_into_bitvec(&mut message_bitvec, 77);

    let t1 = message_bitvec[0];
    let c28_1 = u32::from_bitslice(&message_bitvec[1..29]);
    let c28_2 = u32::from_bitslice(&message_bitvec[29..57]);
    let r1 = message_bitvec[57];
    let r3 = u8::from_bitslice(&message_bitvec[58..61]);
    let s13 = u16::from_bitslice(&message_bitvec[61..74]);
    let message_type = u8::from_bitslice(&message_bitvec[74..77]);

    if message_type != 3 {
        return Err(MessageParseError::InvalidMessage);
    }

    let thank_you = t1;
    
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
    
    let report = match try_parse_r3_to_string(r3.into()) {
        Ok(r) => r,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    
    let serial_or_province = match SerialNumberOrStateOrProvince::try_from_packed_bits(s13) {
        Ok(s) => s,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let thank_you_message = if thank_you { "TU; " } else { "" };
    let ack_message = if ack { "R " } else { "" };
    let packed_string = format!(
        "{}{} {} {}{} {}",
        thank_you_message, callsign1, callsign2, ack_message, report, serial_or_province
    );

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

pub fn try_from_string(value: &str) -> Result<Message, MessageParseError> {
    // parse into words
    let message_words = value.split_whitespace().collect::<Vec<&str>>();
    let mut deq = VecDeque::from_iter(message_words.iter().copied());

    let thank_you = match parse::try_parse_thank_you(&mut deq) {
        Ok(t) => t,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

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

    let ack = match parse::try_parse_ack(&mut deq) {
        Ok(a) => a,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let (report, report_str) = match try_parse_rtty_report(&mut deq) {
        Ok((r, s)) => (r, s),
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let serial_or_province = match try_parse_rtty_serial_or_state(&mut deq) {
        Ok(s) => s,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    // pack all the bits together
    let mut message_bitvec: BitVec = BitVec::new();
    thank_you.pack_into_bitvec(&mut message_bitvec, 1);
    callsign1
        .packed_28bits
        .pack_into_bitvec(&mut message_bitvec, 28);
    callsign2
        .packed_28bits
        .pack_into_bitvec(&mut message_bitvec, 28);
    ack.pack_into_bitvec(&mut message_bitvec, 1);
    report.pack_into_bitvec(&mut message_bitvec, 3);
    serial_or_province
        .packed_bits
        .pack_into_bitvec(&mut message_bitvec, 13);
    3u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = u128::from_bitslice(&message_bitvec[0..77]);

    let thank_you_message = if thank_you { "TU; " } else { "" };
    let ack_message = if ack { "R " } else { "" };
    let packed_string = format!(
        "{}{} {} {}{} {}",
        thank_you_message, callsign1, callsign2, ack_message, report_str, serial_or_province
    );

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

fn try_parse_rtty_report(deq: &mut VecDeque<&str>) -> Result<(u32, String), MessageParseError> {
    if let Some(word) = deq.pop_front() {
        if let Ok(r) = word.parse::<u32>() {
            if (r >= 529 && r <= 599) || (r >= 52 && r <= 59) {
                let second_char = word.chars().nth(1).unwrap().to_string();
                let report = second_char.parse::<u32>().unwrap() - 2;
                let report_str = word.to_string();
                return Ok((report, report_str));
            }
        }
    }
    return Err(MessageParseError::InvalidMessage);
}

fn try_parse_r3_to_string(r3: u32) -> Result<String, MessageParseError> {
    if r3 <= 7 {
        let report = format!("5{}9", r3 + 2);
        return Ok(report.to_string());
    }
    return Err(MessageParseError::InvalidMessage);
}

fn try_parse_rtty_serial_or_state(
    deq: &mut VecDeque<&str>,
) -> Result<SerialNumberOrStateOrProvince, MessageParseError> {
    if let Some(word) = deq.pop_front() {
        if let Ok(s) = SerialNumberOrStateOrProvince::try_from_string(word) {
            return Ok(s);
        }
    }
    return Err(MessageParseError::InvalidMessage);
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_try_from_u128() {
        let message_bits = 0b11001010111000010000100011101000010011011110111100011010111001111101010101011;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "TU; KA0DEF K1ABC R 569 MA");
        assert_eq!(message.message, message_bits);
    }

    #[test]
    fn test_try_from_string() {
        let message_str = "TU; KA0DEF K1ABC R 569 MA";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "TU; KA0DEF K1ABC R 569 MA");
        assert_eq!(message.message, 0b11001010111000010000100011101000010011011110111100011010111001111101010101011);
    }
}