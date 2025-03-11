use bitvec::prelude::*;
use std::collections::VecDeque;

use crate::constants::FT8_CHAR_TABLE_FULL;
use crate::util::bitvec_utils::{bitvec_to_u128, PackBitvecFieldType};
use super::radix::{FromStrCustomRadix, ParseRadixStringError};

use super::{callsign::Callsign, channel_symbols::channel_symbols, checksum::checksum, ldpc::generate_parity, message_parse_error::MessageParseError, report::Report};
use super::{parse, Message};

impl TryFrom<&str> for Message {
    type Error = MessageParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(MessageParseError::EmptyString);
        }

        // parse into words
        let message_words = value.split_whitespace().collect::<Vec<&str>>();

        // standard or EU VHF
        if let Ok(message) = try_from_standard_or_eu_vhf_message(&message_words) {
            return Ok(message);
        }

        // non-standard
        if let Ok(message) = try_from_non_standard_message(&message_words) {
            return Ok(message);
        }

        // RTTY Roundup
        if let Ok(message) = try_from_rtty_roundup_message(&message_words) {
            return Ok(message);
        }

        // DXpedition
        if let Ok(message) = try_from_dxpedition_message(&message_words) {
            return Ok(message);
        }

        // ARRL Field Day
        if let Ok(message) = try_from_arrl_field_day_message(&message_words) {
            return Ok(message);
        }

        // Telemetry
        if let Ok(message) = try_from_telemetry_message(value) {
            return Ok(message);
        }

        // free text message
        if let Ok(message) = try_from_free_text_string(value) {
            return Ok(message);
        }

        // unable to parse
        return Err(MessageParseError::InvalidMessage);
    }
}

impl TryFrom<String> for Message {
    type Error = MessageParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        return Message::try_from(value.as_str());
    }
}

fn try_from_standard_or_eu_vhf_message(
    message_words: &[&str],
) -> Result<Message, MessageParseError> {
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
    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
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
    let packed_callsign1: String;
    if callsign1.is_hashed {
        packed_callsign1 = format!("<{}>", callsign1.callsign);
    } else {
        packed_callsign1 = format!("{}", callsign1.callsign);
    }
    let packed_callsign2: String;
    if callsign2.is_hashed {
        packed_callsign2 = format!("<{}>", callsign2.callsign);
    } else {
        packed_callsign2 = format!("{}", callsign2.callsign);
    }
    let packed_string = format!(
        "{} {} {}",
        packed_callsign1, packed_callsign2, report.report
    );

    let checksum = checksum(message);
    let parity = generate_parity(message, checksum);
    let channel_symbols = channel_symbols(message, checksum, parity);

    Ok(Message {
        message,
        checksum,
        parity,
        channel_symbols,
        display_string: packed_string.trim().to_string(),
    })
}

fn try_from_non_standard_message(message_words: &[&str]) -> Result<Message, MessageParseError> {
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
    let message = bitvec_to_u128(&message_bitvec, 77);

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

    let checksum = checksum(message);
    let parity = generate_parity(message, checksum);
    let channel_symbols = channel_symbols(message, checksum, parity);

    Ok(Message {
        message,
        parity,
        checksum,
        channel_symbols,
        display_string: packed_string,
    })
}

fn try_from_rtty_roundup_message(message_words: &[&str]) -> Result<Message, MessageParseError> {
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

    let (report, report_str) = match parse::try_parse_rtty_report(&mut deq) {
        Ok((r, s)) => (r, s),
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let serial_or_province = match parse::try_parse_rtty_serial_or_state(&mut deq) {
        Ok(s) => s,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    // pack all the bits together
    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
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
    let message = bitvec_to_u128(&message_bitvec, 77);

    let thank_you_message = if thank_you { "TU; " } else { "" };
    let ack_message = if ack { "R " } else { "" };
    let packed_string = format!(
        "{}{} {} {}{} {}",
        thank_you_message, callsign1, callsign2, ack_message, report_str, serial_or_province
    );

    let checksum = checksum(message);
    let parity = generate_parity(message, checksum);
    let channel_symbols = channel_symbols(message, checksum, parity);

    Ok(Message {
        message,
        checksum,
        parity,
        channel_symbols,
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

fn try_from_dxpedition_message(message_words: &[&str]) -> Result<Message, MessageParseError> {
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
    let message = bitvec_to_u128(&message_bitvec, 77);

    // DXpedition K1ABC RR73; W9XYZ <KH1/KH7Z> -08
    let packed_string = format!(
        "{} RR73; {} <{}> {}",
        callsign1, callsign2, callsign3, report_str
    );

    let checksum = checksum(message);
    let parity = generate_parity(message, checksum);
    let channel_symbols = channel_symbols(message, checksum, parity);

    Ok(Message {
        message,
        checksum,
        parity,
        channel_symbols,
        display_string: packed_string,
    })
}

fn try_from_arrl_field_day_message(message_words: &[&str]) -> Result<Message, MessageParseError> {
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
    ) = match parse::try_parse_transmitters_and_class(&mut deq) {
        Ok((n, ns, s, f, fs)) => (n, ns, s, f, fs),
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let section = match parse::try_parse_arrl_section(&mut deq) {
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
    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
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
    let message = bitvec_to_u128(&message_bitvec, 77);

    // Field Day    W9XYZ K1ABC R 17B EMA   c28 c28 R1 n4 k3 S7
    let ack_string = if ack { "R " } else { "" };
    let packed_string = format!("{callsign1} {callsign2} {ack_string}{number_transmitters_string}{field_day_class_string} {section}");

    let checksum = checksum(message);
    let parity = generate_parity(message, checksum);
    let channel_symbols = channel_symbols(message, checksum, parity);

    Ok(Message {
        message,
        checksum,
        parity,
        channel_symbols,
        display_string: packed_string,
    })
}

fn try_from_telemetry_message(message_string: &str) -> Result<Message, MessageParseError> {
    if message_string.len() > 18 {
        return Err(MessageParseError::InvalidMessage);
    }

    let value = match u128::from_str_radix(message_string, 16) {
        Ok(v) => v,
        Err(_) => {
            return Err(MessageParseError::InvalidMessage);
        }
    };

    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
    value.pack_into_bitvec(&mut message_bitvec, 71);
    5u8.pack_into_bitvec(&mut message_bitvec, 3);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = bitvec_to_u128(&message_bitvec, 77);

    let checksum = checksum(message);
    let parity = generate_parity(message, checksum);
    let channel_symbols = channel_symbols(message, checksum, parity);

    Ok(Message {
        message,
        checksum,
        parity,
        channel_symbols,
        display_string: message_string.to_owned(),
    })
}

fn try_from_free_text_string(message_string: &str) -> Result<Message, MessageParseError> {
    // the string to pack into bits must be 13 characters
    //   right align with spaces if needed
    //   trim to length if needed
    let adjusted_string: String = format!("{: >13.13}", message_string);
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
    let mut message_bitvec: BitVec<u8, Msb0> = BitVec::new();
    f71.pack_into_bitvec(&mut message_bitvec, 71);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    0u8.pack_into_bitvec(&mut message_bitvec, 3);
    let message = bitvec_to_u128(&message_bitvec, 77);

    let checksum = checksum(message);
    let parity = generate_parity(message, checksum);
    let channel_symbols = channel_symbols(message, checksum, parity);

    Ok(Message {
        message,
        display_string: packed_string,
        checksum,
        parity,
        channel_symbols,
    })
}