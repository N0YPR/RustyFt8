use std::collections::VecDeque;
use bitvec::prelude::*;
use crate::util::bitvec_utils::{FromBitSlice, bitvec_to_u128, PackBitvecFieldType};

use super::{callsign::Callsign, message_parse_error::MessageParseError, parse, report::Report, Message};

pub fn try_from_u128(message: u128) -> Result<Message, MessageParseError> {
    let mut message_bitvec: BitVec = BitVec::new();
    message.pack_into_bitvec(&mut message_bitvec, 77);

    let callsign1_bits = u32::from_bitslice(&message_bitvec[0..28]);
    let is_rover1 = message_bitvec[28];
    let callsign2_bits = u32::from_bitslice(&message_bitvec[29..57]);
    let is_rover2 = message_bitvec[57];
    let is_ack = message_bitvec[58];
    let report_bits = u32::from_bitslice(&message_bitvec[59..74]);
    let message_type = u8::from_bitslice(&message_bitvec[74..77]);

    if message_type != 1 && message_type != 2 {
        return Err(MessageParseError::InvalidMessage);
    }    

    let mut callsign1 = match Callsign::try_from(callsign1_bits) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    if message_type == 1 {
        callsign1.is_rover = is_rover1
    } else {} {
        callsign1.is_portable = is_rover1;
    }

    let mut callsign2 = match Callsign::try_from(callsign2_bits) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    if message_type == 1 {
        callsign2.is_rover = is_rover2
    } else {} {
        callsign2.is_portable = is_rover2;
    }

    let mut report = match Report::try_from_packed_bits(report_bits, 15) {
        Ok(r) => r,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    report.is_ack = is_ack;
    println!("report: {:?}", report);
    println!("report: {}", report);

    // pack the string
    let display_string = pack_string(callsign1, callsign2, report);

    Ok(Message {
        message,
        display_string,
    })
}

pub fn try_from_string(value: &str) -> Result<Message, MessageParseError> {
    // parse into words
    let message_words = value.split_whitespace().collect::<Vec<&str>>();

    if message_words.len() < 2 {
        return Err(MessageParseError::InvalidMessage);
    }

    let mut deq = VecDeque::from_iter(message_words.iter().copied());

    let callsign1 = match parse::try_parse_callsign(&mut deq) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    if callsign1.was_hashed && !callsign1.is_hashed {
        return Err(MessageParseError::InvalidMessage);
    }

    let callsign2 = match parse::try_parse_callsign(&mut deq) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    if callsign2.was_hashed && !callsign2.is_hashed {
        return Err(MessageParseError::InvalidMessage);
    }

    let report = match parse::try_parse_standard_report(&mut deq) {
        Ok(r) => r,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    // cases where the message is actually a non-standard callsign message
    if callsign1.callsign.starts_with("CQ") && callsign2.is_hashed {
        return Err(MessageParseError::InvalidMessage);
    }

    if !deq.is_empty() {
        return Err(MessageParseError::InvalidMessage);
    }

    // pack all the bits together
    let mut message_bitvec: BitVec = BitVec::new();
    let message_type: u8;
    if callsign1.is_portable || callsign2.is_portable {
        // EU VHF
        message_type = 2;

        callsign1
            .packed_28bits
            .pack_into_bitvec(&mut message_bitvec, 28);
        callsign1
            .is_portable
            .pack_into_bitvec(&mut message_bitvec, 1);
        callsign2
            .packed_28bits
            .pack_into_bitvec(&mut message_bitvec, 28);
        callsign2
            .is_portable
            .pack_into_bitvec(&mut message_bitvec, 1);
        report.is_ack.pack_into_bitvec(&mut message_bitvec, 1);
        report.packed_bits.pack_into_bitvec(&mut message_bitvec, 15);
        message_type.pack_into_bitvec(&mut message_bitvec, 3);
    } else {
        // Standard
        message_type = 1;

        callsign1
            .packed_28bits
            .pack_into_bitvec(&mut message_bitvec, 28);
        callsign1.is_rover.pack_into_bitvec(&mut message_bitvec, 1);
        callsign2
            .packed_28bits
            .pack_into_bitvec(&mut message_bitvec, 28);
        callsign2.is_rover.pack_into_bitvec(&mut message_bitvec, 1);
        report.is_ack.pack_into_bitvec(&mut message_bitvec, 1);
        report.packed_bits.pack_into_bitvec(&mut message_bitvec, 15);
        message_type.pack_into_bitvec(&mut message_bitvec, 3);
    }
    let message = bitvec_to_u128(&message_bitvec, 77);

    // pack the string
    let display_string = pack_string(callsign1, callsign2, report);

    Ok(Message {
        message,
        display_string,
    })
}

fn pack_string(callsign1: Callsign, callsign2: Callsign, report: Report) -> String {
    let mut packed_callsign1: String;
    if callsign1.is_hashed {
        packed_callsign1 = format!("<{}>", callsign1);
    } else {
        packed_callsign1 = format!("{}", callsign1.to_string(),);
    }
  
    let packed_callsign2: String;
    if callsign2.is_hashed {
        packed_callsign2 = format!("<{}>", callsign2);
    } else {
        packed_callsign2 = format!("{}", callsign2);
    }
    
    let packed_string = format!(
        "{} {} {}",
        packed_callsign1, packed_callsign2, report
    );
    let packed_string = packed_string.trim().to_string();
    packed_string
}

#[cfg(test)]
mod tests {
    use crate::message;

    use super::*;

    #[test]
    fn test_try_from_u128() {
        let message_bits = 0b00000000010111100101100110000000010100100110110011100110110001100111110010001;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "CQ SOTA N0YPR/R DM42");
    }

    #[test]
    fn test_try_from_string() {
        let message_str = "CQ SOTA N0YPR/R DM42";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "CQ SOTA N0YPR/R DM42");
        assert_eq!(message.message, 0b00000000010111100101100110000000010100100110110011100110110001100111110010001);
    }

    #[test]
    fn test_try_from_u128_2() {
        let message_bits = 0b00001100001010010011101110000000010011011110111100011010100111111010101000001;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "W9XYZ K1ABC -11");
    }

    #[test]
    fn test_try_from_u128_eu() {
        let message_bits = 0b10110111101110101100010101000000010010000110000010110011010111111001110101010;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "PA9XYZ G4ABC/P RR73");
    }

    #[test]
    fn test_try_from_string_eu() {
        let message_str = "PA9XYZ G4ABC/P RR73";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "PA9XYZ G4ABC/P RR73");
        assert_eq!(message.message, 0b10110111101110101100010101000000010010000110000010110011010111111001110101010);
    }

    #[test]
    fn test_try_from_u128_3() {
        // pre-cache all possible hashed callsigns
        let callsign1 = Callsign::from_callsign_str("PJ4/K1ABC").expect("callsign should have been cached");
        let callsign2 = Callsign::from_callsign_str("W9XYZ").expect("callsign should have been cached");

        let message_bits = 0b00001100001010010011101110000000000110101001010110000101000111111010101000001;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "W9XYZ <PJ4/K1ABC> -11");
    }
}