use bitvec::prelude::*;

use crate::util::bitvec_utils::{FromBitSlice, PackBitvecFieldType};

use super::{callsign::Callsign, message_parse_error::MessageParseError, report::Report, Message};

pub fn try_from_u128(message: u128) -> Result<Message, MessageParseError> {
    // https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
    // Type i3.n3 Purpose Example message Bit-fi eld tags
    // 4. NonStd Call <W9XYZ> PJ4/K1ABC RRR h12 c58 h1 r2 c1
    // h12 Hashed callsign, 12 bits
    // c58 Nonstandard callsign, up to 11 characters
    // h1 Hashed callsign is the second callsign
    // r2 RRR, RR73, 73, or blank
    // c1 First callsign is CQ; h12 is ignored
    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
    message.pack_into_bitvec(&mut message_bitvec, 77);

    let h12 = u32::from_bitslice(&message_bitvec[0..12]);
    let c58 = u64::from_bitslice(&message_bitvec[12..70]);
    let h1 = message_bitvec[70];
    let r2 = u8::from_bitslice(&message_bitvec[71..73]);
    let c1 = message_bitvec[73];
    let message_type = u8::from_bitslice(&message_bitvec[74..77]);
    
    if message_type != 4 {
        return Err(MessageParseError::InvalidMessage);
    }

    let callsign1:Callsign;
    let callsign2:Callsign;
    if c1 {
        // First callsign is CQ; h12 is ignored
        callsign1 = Callsign::from_callsign_str("CQ").unwrap();
        callsign2 = match Callsign::try_from(c58) {
            Ok(c) => c,
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
    } else if h1 {
        // Hashed callsign is the second callsign
        callsign1 = match Callsign::try_from(c58) {
            Ok(c) => c,
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
        callsign2 = match Callsign::try_from_callsign_hash(h12) {
            Ok(c) => c,
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
    } else {
        callsign1 = match Callsign::try_from_callsign_hash(h12) {
            Ok(c) => c,
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
        callsign2 = match Callsign::try_from(c58) {
            Ok(c) => c,
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
    }

    let packed_string: String;
    let report: Report;
    if c1 {
        // First callsign is CQ; h12 is ignored
        packed_string = format!("CQ {}", callsign2.callsign);
    } else {
        report = match Report::try_from_packed_2(r2.into()) {
            Ok(r) => r,
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
        if !report.is_other {
            return Err(MessageParseError::InvalidMessage);
        }
        if !h1 {
            packed_string = format!("<{}> {} {}", callsign1, callsign2, report)
                .trim()
                .to_string();
        } else {
            packed_string = format!("{} <{}> {}", callsign1, callsign2, report)
                .trim()
                .to_string();
        }
    }

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

pub fn try_from_string(value: &str) -> Result<Message, MessageParseError> {
    // parse into words
    let message_words = value.split_whitespace().collect::<Vec<&str>>();

    // must have at least 2? words in order to be a standard message
    if message_words.len() < 2 {
        return Err(MessageParseError::InvalidMessage);
    }
    let h12: u128;
    let h1: u128;
    let c58: u128;
    let r2: u128;
    let c1: u128 = (message_words[0] == "CQ") as u128;

    let callsign1: Callsign;
    let callsign2: Callsign;
    let report: Report;

    if message_words.len() == 2 {
        if message_words[0] == "CQ" {
            if let Ok(callsign) = Callsign::from_callsign_str(message_words[1]) {
                callsign1 = Callsign::from_callsign_str("CQ").unwrap();
                callsign2 = callsign;
                h12 = callsign2.hashed_12bits as u128;
                h1 = 0;
                c58 = callsign2.packed_58bits as u128;
                r2 = 0;
                report = Report::try_from_report_str("").unwrap();
            } else {
                return Err(MessageParseError::InvalidMessage);
            }
        } else {
            if let Ok(callsign) = Callsign::from_callsign_str(message_words[0]) {
                callsign1 = callsign;
            } else {
                return Err(MessageParseError::InvalidMessage);
            }

            if let Ok(callsign) = Callsign::from_callsign_str(message_words[1]) {
                callsign2 = callsign;
            } else {
                return Err(MessageParseError::InvalidMessage);
            }

            report = Report::try_from_report_str("").unwrap();
            r2 = 0;

            if callsign1.is_hashed {
                c58 = callsign1.packed_58bits.into();
                h12 = callsign2.hashed_12bits.into();
                h1 = 1;
            } else if callsign2.is_hashed {
                c58 = callsign2.packed_58bits.into();
                h12 = callsign1.hashed_12bits.into();
                h1 = 0;
            } else {
                return Err(MessageParseError::InvalidMessage);
            }
        }
    } else if message_words.len() == 3 {
        if let Ok(callsign) = Callsign::from_callsign_str(message_words[0]) {
            callsign1 = callsign;
        } else {
            return Err(MessageParseError::InvalidMessage);
        }

        if let Ok(callsign) = Callsign::from_callsign_str(message_words[1]) {
            callsign2 = callsign;
        } else {
            return Err(MessageParseError::InvalidMessage);
        }

        if callsign1.was_hashed {
            c58 = callsign2.packed_58bits.into();
            h12 = callsign1.hashed_12bits.into();
            h1 = 0;
        } else if callsign2.was_hashed {
            c58 = callsign1.packed_58bits.into();
            h12 = callsign2.hashed_12bits.into();
            h1 = 1;
        } else {
            return Err(MessageParseError::InvalidMessage);
        }

        if let Ok(rpt) = Report::try_from_report_str(message_words[2]) {
            if rpt.is_other {
                report = rpt;
                r2 = report.other_bits.into();
            } else {
                return Err(MessageParseError::InvalidMessage);
            }
        } else {
            return Err(MessageParseError::InvalidMessage);
        }
    } else {
        return Err(MessageParseError::InvalidMessage);
    }

    // pack all the bits together
    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
    h12.pack_into_bitvec(&mut message_bitvec, 12);
    c58.pack_into_bitvec(&mut message_bitvec, 58);
    h1.pack_into_bitvec(&mut message_bitvec, 1);
    r2.pack_into_bitvec(&mut message_bitvec, 2);
    c1.pack_into_bitvec(&mut message_bitvec, 1);
    4u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = u128::from_bitslice(&message_bitvec[0..77]);

    let packed_string: String;
    if message_words[0] == "CQ" {
        packed_string = format!("CQ {}", callsign2.callsign);
    } else {
        if h1 == 0 {
            packed_string = format!("<{}> {} {}", callsign1, callsign2, report)
                .trim()
                .to_string();
        } else {
            packed_string = format!("{} <{}> {}", callsign1, callsign2, report)
                .trim()
                .to_string();
        }
    }

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

//<W9XYZ> PJ4/K1ABC RRR

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_from_u128() {
        // ensure callsigns have already been cached
        let _callsign1 = Callsign::from_callsign_str("PJ4/K1ABC").expect("callsign should have been cached");
        let _callsign2 = Callsign::from_callsign_str("W9XYZ").expect("callsign should have been cached");

        let message_bits = 0b11110011000100000000000110100011101000110001000111001010101000000000010010100;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "<W9XYZ> PJ4/K1ABC RRR");
    }

    #[test]
    fn test_try_from_string() {
        // ensure callsigns have already been cached
        let _callsign1 = Callsign::from_callsign_str("PJ4/K1ABC").expect("callsign should have been cached");
        let _callsign2 = Callsign::from_callsign_str("W9XYZ").expect("callsign should have been cached");

        let message_str = "<W9XYZ> PJ4/K1ABC RRR";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "<W9XYZ> PJ4/K1ABC RRR");
        assert_eq!(message.message, 0b11110011000100000000000110100011101000110001000111001010101000000000010010100);
    }
}