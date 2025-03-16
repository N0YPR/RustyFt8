use bitvec::prelude::*;

use std::collections::VecDeque;


use crate::util::bitvec_utils::{FromBitSlice, PackBitvecFieldType};
use super::{callsign::Callsign, message_parse_error::MessageParseError, parse, Message};

pub fn try_from_u128(message: u128) -> Result<Message, MessageParseError> {
    // https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf
    // Type i3.n3 Purpose Example message Bit-fi eld tags
    // 0.1 DXpedition K1ABC RR73; W9XYZ <KH1/KH7Z> -08 c28 c28 h10 r5
    // c28 Standard callsign, CQ, DE, QRZ, or 22-bit hash
    // c28 Standard callsign, CQ, DE, QRZ, or 22-bit hash
    // h10 Hashed callsign, 10 bits
    // r5 Report: -30 to +32, even numbers only
    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
    message.pack_into_bitvec(&mut message_bitvec, 77);

    let c28_1 = u32::from_bitslice(&message_bitvec[0..28]);
    let c28_2 = u32::from_bitslice(&message_bitvec[28..56]);
    let h10 = u16::from_bitslice(&message_bitvec[56..66]);
    let r5 = u8::from_bitslice(&message_bitvec[66..71]);
    let message_subtype = u8::from_bitslice(&message_bitvec[71..74]);
    let message_type = u8::from_bitslice(&message_bitvec[74..77]);

    println!(
        "c28_1: {}, c28_2: {}, h10: {}, r5: {}, message_type: {}, message_subtype: {}",
        c28_1, c28_2, h10, r5, message_type, message_subtype
    );

    // check message type
    if message_type != 0 || message_subtype != 1 {
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

    let callsign3 = match Callsign::try_from_callsign_hash(h10.into()) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let report = match try_parse_r5(r5) {
        Ok(r) => r,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };
    
    let packed_string = format!(
        "{} RR73; {} <{}> {}",
        callsign1, callsign2, callsign3, report
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
    
    let callsign1: Callsign = match parse::try_parse_callsign(&mut deq) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    match deq.pop_front() {
        Some(word) => {
            if word != "RR73;" {
                return Err(MessageParseError::InvalidMessage);
            }
        }
        None => {
            return Err(MessageParseError::InvalidMessage);
        }
    }

    let callsign2: Callsign = match parse::try_parse_callsign(&mut deq) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let callsign3: Callsign = match parse::try_parse_callsign(&mut deq) {
        Ok(c) => c,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let (report, report_str) = match try_parse_dxpedition_signal_report(&mut deq) {
        Ok((r, s)) => (r, s),
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
    callsign1
        .packed_28bits
        .pack_into_bitvec(&mut message_bitvec, 28);
    callsign2
        .packed_28bits
        .pack_into_bitvec(&mut message_bitvec, 28);
    callsign3
        .hashed_10bits
        .pack_into_bitvec(&mut message_bitvec, 10);
    report.pack_into_bitvec(&mut message_bitvec, 5);
    1u8.pack_into_bitvec(&mut message_bitvec, 3);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = u128::from_bitslice(&message_bitvec[0..77]);

    // DXpedition K1ABC RR73; W9XYZ <KH1/KH7Z> -08
    let packed_string = format!(
        "{} RR73; {} <{}> {}",
        callsign1, callsign2, callsign3, report_str
    );

    Ok(Message {
        message,
        display_string: packed_string,
    })
}

fn try_parse_dxpedition_signal_report(
    deq: &mut VecDeque<&str>,
) -> Result<(u8, String), MessageParseError> {
    if let Some(word) = deq.pop_front() {
        if let Ok(value) = i8::from_str_radix(word, 10) {
            if value < -30 || value > 30 {
                return Err(MessageParseError::InvalidMessage);
            }
            let report = (value + 30) as u8 / 2;
            return Ok((report, word.to_string()));
        }
    }
    return Err(MessageParseError::InvalidMessage);
}

fn try_parse_r5(r5: u8) -> Result<String, MessageParseError> {
    if r5 > 31 {
        return Err(MessageParseError::InvalidMessage);
    }
    let report = (r5 as i8 * 2) - 30;
    Ok(format!("{:+03}", report))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_try_from_u128() {
        // ensure callsigns have already been cached
        let _callsign1 = Callsign::from_callsign_str("K1ABC").expect("callsign should have been cached");
        let _callsign2 = Callsign::from_callsign_str("KH1/KH7Z").expect("callsign should have been cached");

        let message_bits = 0b00001001101111011110001101010000110000101001001110111000001100100101011001000;
        let message = try_from_u128(message_bits).expect("should have been able to parse");
        assert_eq!(message.display_string, "K1ABC RR73; W9XYZ <KH1/KH7Z> -08");
        assert_eq!(message.message, message_bits);
    }

    #[test]
    fn test_try_from_string() {
        // ensure callsigns have already been cached
        let _callsign1 = Callsign::from_callsign_str("K1ABC").expect("callsign should have been cached");
        let _callsign2 = Callsign::from_callsign_str("KH1/KH7Z").expect("callsign should have been cached");
        
        let message_str = "K1ABC RR73; W9XYZ <KH1/KH7Z> -08";
        let message = try_from_string(message_str).expect("Should have been able to parse");
        assert_eq!(message.display_string, "K1ABC RR73; W9XYZ <KH1/KH7Z> -08");
        assert_eq!(message.message, 0b00001001101111011110001101010000110000101001001110111000001100100101011001000);
    }
}