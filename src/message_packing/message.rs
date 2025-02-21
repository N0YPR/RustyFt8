use std::collections::VecDeque;
use std::fmt::Display;

use snafu::Snafu;

use crate::message_packing::constants::*;
use crate::message_packing::radix:: {FromStrCustomRadix, ParseRadixStringError};
use crate::message_packing::callsign::Callsign;

use super::arrl_section::ArrlSection;
use super::report::Report;
use super::serial_number_or_state_or_province::SerialNumberOrStateOrProvince;

#[derive(Debug)]
pub struct Message {
    packed_bits: u128,
    display_string: String,
    message_type: u8,
    message_subtype: u8,
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string)
    }
}

#[derive(Debug, Snafu)]
pub enum MessageParseError {
    /// String contains invalid character
    #[snafu(display("message_string contains an invalid character"))]
    InvalidChar,

    /// String could not be parsed as a valid message
    #[snafu(display("message_string could not be parsed as a valid message"))]
    InvalidMessage,

    /// Empty String
    #[snafu(display("message_string empty"))]
    EmptyString,
}

impl TryFrom<&str> for Message {
    type Error = MessageParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(MessageParseError::EmptyString);
        }

        // parse into words
        let message_words = value.split_whitespace().collect::<Vec<&str>>();

        // standard or EU VHF
        if let Ok(message) = Self::try_from_standard_or_eu_vhf_message(&message_words) {
            return Ok(message);
        }

        // non-standard
        if let Ok(message) = Self::try_from_non_standard_message(&message_words) {
            return Ok(message);
        }

        // RTTY Roundup
        if let Ok(message) = Self::try_from_rtty_roundup_message(&message_words) {
            return Ok(message);
        }

        // DXpedition
        if let Ok(message) = Self::try_from_dxpedition_message(&message_words) {
            return Ok(message);
        }

        // ARRL Field Day
        if let Ok(message) = Self::try_from_arrl_field_day_message(&message_words) {
            return Ok(message);
        }

        // Telemetry
        if let Ok(message) = Self::try_from_telemetry_message(value) {
            return Ok(message);
        }

        // free text message
        if let Ok(message) = Self::try_from_free_text_string(value) {
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

impl From<Message> for u128 {
    fn from(value: Message) -> Self {
        return value.packed_bits;
    }
}

impl Message {
    fn try_parse_standard_report(deq:&mut VecDeque<&str>) -> Result<Report, MessageParseError> {
        if let Some(word) = deq.pop_front() {
            let word_to_parse:String;
            if word == "R" {
                if let Some(next_word) = deq.pop_front() {
                    word_to_parse = format!("{} {}", word, next_word)
                } else {
                    return Err(MessageParseError::InvalidMessage);
                }
            } else {
                word_to_parse = word.to_string();
            }
            match Report::try_from_report_str(&word_to_parse) {
                Ok(c) => { return Ok(c) },
                Err(_) => { return Err(MessageParseError::InvalidMessage); }
            };
        } else {
            match Report::try_from_report_str("") {
                Ok(r) => {return Ok(r) },
                Err(_) => { return Err(MessageParseError::InvalidMessage); }
            }; 
        }
    }

    fn try_from_standard_or_eu_vhf_message(message_words:&[&str]) -> Result<Self, MessageParseError> {
        if message_words.len() < 2 {
            return Err(MessageParseError::InvalidMessage);
        }

        let mut deq = VecDeque::from_iter(message_words.iter().copied());

        let callsign1 = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };
        if callsign1.was_hashed && !callsign1.is_hashed {
            return Err(MessageParseError::InvalidMessage);
        }

        let callsign2 = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };
        if callsign2.was_hashed && !callsign2.is_hashed {
            return Err(MessageParseError::InvalidMessage);
        }

        let report = match Self::try_parse_standard_report(&mut deq) {
            Ok(r) => r,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        // cases where the message is actually a non-standard callsign message
        if callsign1.callsign.starts_with("CQ") && callsign2.is_hashed {
            return Err(MessageParseError::InvalidMessage);
        }

        if !deq.is_empty() {
            return Err(MessageParseError::InvalidMessage);
        }

        // pack all the bits together
        let packed_bits:u128;
        let message_type:u8;
        if callsign1.is_portable || callsign2.is_portable {
            // EU VHF
            message_type = 2;
            packed_bits = Self::pack_bits(&[
                (callsign1.packed_28bits.into(), 28),
                (callsign1.is_portable.into(), 1),
                (callsign2.packed_28bits.into(), 28),
                (callsign2.is_portable.into(), 1),
                (report.is_ack.into(), 1),
                (report.packed_bits.into(), 15),
                (2_u128, 3),
            ]);
        }
        else {
            // Standard
            message_type = 1;
            packed_bits = Self::pack_bits(&[
                (callsign1.packed_28bits.into(), 28),
                (callsign1.is_rover.into(), 1),
                (callsign2.packed_28bits.into(), 28),
                (callsign2.is_rover.into(), 1),
                (report.is_ack.into(), 1),
                (report.packed_bits.into(), 15),
                (1u128, 3),
            ]);
        }        

        // pack the string
        let packed_callsign1:String;
        if callsign1.is_hashed {
            packed_callsign1 = format!("<{}>", callsign1.callsign);
        } else {
            packed_callsign1 = format!("{}", callsign1.callsign);
        }
        let packed_callsign2:String;
        if callsign2.is_hashed {
            packed_callsign2 = format!("<{}>", callsign2.callsign);
        } else {
            packed_callsign2 = format!("{}", callsign2.callsign);
        }
        let packed_string = format!("{} {} {}", packed_callsign1, packed_callsign2, report.report);

        Ok(Message {
            packed_bits,
            display_string: packed_string.trim().to_string(),
            message_type: message_type,
            message_subtype: 0
        })
    }

    fn try_from_non_standard_message(message_words:&[&str]) -> Result<Self, MessageParseError> {
        // must have at least 2? words in order to be a standard message
        if message_words.len() < 2 {
            return Err(MessageParseError::InvalidMessage);
        }

        let h12:u128;
        let h1:u128;
        let c58:u128;
        let r2:u128;
        let c1:u128 = (message_words[0] == "CQ") as u128;

        let callsign1:Callsign;
        let callsign2:Callsign;
        let report:Report;

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
        let packed_bits = Self::pack_bits(&[
            (h12.into(), 12),
            (c58.into(), 58),
            (h1.into(), 1),
            (r2.into(), 2),
            (c1.into(), 1),
            (4u128, 3),
        ]);

        let packed_string:String;
        if message_words[0] == "CQ" {
            packed_string = format!("CQ {}", callsign2.callsign);
        } else {
            if h1 == 0 {
                packed_string = format!("<{}> {} {}", callsign1, callsign2, report).trim().to_string();
            } else {
                packed_string = format!("{} <{}> {}", callsign1, callsign2, report).trim().to_string();
            }
        }

        Ok(Message {
            packed_bits,
            display_string: packed_string,
            message_type: 4,
            message_subtype: 0
        })
    }
    
    fn try_parse_thank_you(deq:&mut VecDeque<&str>) -> Result<bool, MessageParseError> {
        match deq.pop_front() {
            Some(word) => {
                let tu = word == "TU;";
                if !tu {
                    deq.push_front(word);
                }
                return Ok(tu);
            },
            None => { return Err(MessageParseError::InvalidMessage); }
        }
    }
    
    fn try_parse_rtty_report(deq:&mut VecDeque<&str>) -> Result<(u32, String), MessageParseError> {
        if let Some(word) = deq.pop_front() {
            if let Ok(r) = word.parse::<u32>() {
                if (r >= 529 && r <= 599) || (r >= 52 && r <= 59)  {
                    let second_char = word.chars().nth(1).unwrap().to_string();
                    let report = second_char.parse::<u32>().unwrap() - 2;
                    let report_str = word.to_string();
                    return Ok((report, report_str));
                }
            }
        }
        return Err(MessageParseError::InvalidMessage);
    }
    
    fn try_parse_rtty_serial_or_state(deq:&mut VecDeque<&str>) -> Result<SerialNumberOrStateOrProvince, MessageParseError> {
        if let Some(word) = deq.pop_front() {
            if let Ok(s) = SerialNumberOrStateOrProvince::try_from_string(word) {
                return Ok(s);
            }
        }
        return Err(MessageParseError::InvalidMessage);
    }

    fn try_from_rtty_roundup_message(message_words:&[&str]) -> Result<Self, MessageParseError> {
        let mut deq = VecDeque::from_iter(message_words.iter().copied());

        let thank_you = match Self::try_parse_thank_you(&mut deq) {
            Ok(t) => t,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let callsign1:Callsign = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let callsign2:Callsign = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let ack = match Self::try_parse_ack(&mut deq) {
            Ok(a) => a,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let (report, report_str) = match Self::try_parse_rtty_report(&mut deq) {
            Ok((r, s)) => (r,s),
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let serial_or_province = match Self::try_parse_rtty_serial_or_state(&mut deq) {
            Ok(s) => s,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        // pack all the bits together
        let packed_bits = Self::pack_bits(&[
            (thank_you.into(), 1),
            (callsign1.packed_28bits.into(), 28),
            (callsign2.packed_28bits.into(), 28),
            (ack.into(), 1),
            (report.into(), 3),
            (serial_or_province.packed_bits.into(), 13),
            (3u128, 3),
        ]);

        let thank_you_message = if thank_you {"TU; "} else {""};
        let ack_message = if ack {"R "} else {""};
        let packed_string = format!("{}{} {} {}{} {}", thank_you_message, callsign1, callsign2, ack_message, report_str, serial_or_province);

        Ok(Message {
            packed_bits,
            display_string: packed_string,
            message_type: 3,
            message_subtype: 0
        })
    }

    fn try_parse_dxpedition_signal_report(deq:&mut VecDeque<&str>) -> Result<(u8,String), MessageParseError> {
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

    fn try_from_dxpedition_message(message_words:&[&str]) -> Result<Self, MessageParseError> {
        let mut deq = VecDeque::from_iter(message_words.iter().copied());

        let callsign1:Callsign = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        match deq.pop_front() {
            Some(word) => {
                if word != "RR73;" {
                    return Err(MessageParseError::InvalidMessage);
                }
            },
            None => { return Err(MessageParseError::InvalidMessage); }
        }

        let callsign2:Callsign = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let callsign3:Callsign = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let (report, report_str) = match Self::try_parse_dxpedition_signal_report(&mut deq) {
            Ok((r,s)) => (r,s),
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let packed_bits = Self::pack_bits(&[
            (callsign1.packed_28bits.into(), 28),
            (callsign2.packed_28bits.into(), 28),
            (callsign3.hashed_10bits.into(), 10),
            (report.into(), 5),
            (1u128, 3),
            (0u128, 3),
        ]);

        // DXpedition K1ABC RR73; W9XYZ <KH1/KH7Z> -08 
        let packed_string = format!("{} RR73; {} <{}> {}", callsign1, callsign2, callsign3, report_str);

        Ok(Message {
            packed_bits,
            display_string: packed_string,
            message_type: 0,
            message_subtype: 1
        })
    }

    fn try_parse_callsign(deq:&mut VecDeque<&str>) -> Result<Callsign, MessageParseError> {
        if let Some(word) = deq.pop_front() {
            if word == "CQ" {
                if let Some(next_word) = deq.pop_front() {
                    if let Ok(callsign) = Callsign::from_callsign_str(&format!("{} {}", word, next_word)) {
                        return Ok(callsign);
                    }
                    deq.push_front(next_word);
                }
            }
            if let Ok(callsign) = Callsign::from_callsign_str(word) {
                return Ok(callsign);
            }
        }
        return Err(MessageParseError::InvalidMessage);
    }

    fn try_parse_ack(deq:&mut VecDeque<&str>) -> Result<bool, MessageParseError> {
        if let Some(word) = deq.pop_front() {
            let ack = word == "R";
            if !ack {
                deq.push_front(word);
            }
            return Ok(ack);
        }
        return Err(MessageParseError::InvalidMessage);
    }

    fn try_parse_arrl_section(deq:&mut VecDeque<&str>) -> Result<ArrlSection, MessageParseError> {
        if let Some(word) = deq.pop_front() {
            return match ArrlSection::try_from_string(word) {
                Ok(s) => Ok(s),
                Err(_) => Err(MessageParseError::InvalidMessage)
            }
        }
        return Err(MessageParseError::InvalidMessage);
    }

    fn all_except_last(value: &str) -> &str {
        let mut chars = value.chars();
        chars.next_back();
        chars.as_str()
    }
    
    fn last(value: &str) -> &str {
        &value[value.len()-1..]
    }

    fn try_parse_transmitters_and_class(deq:&mut VecDeque<&str>) -> Result<(u8, String, u8, u8, String), MessageParseError> {
        if let Some(word) = deq.pop_front() {
            let num_transmitters_string = Self::all_except_last(word).to_string();
            let (num_transmitters, sub_type) = match u8::from_str_radix(&num_transmitters_string, 10) {
                Ok(value) => {
                    if value < 1 || value > 32 {
                        return Err(MessageParseError::InvalidMessage);
                    }
                    if value <= 16 {
                        (value - 1, 3u8)
                    } else {
                        (value - 16 - 1, 4u8)
                    }
                },
                Err(_) => { return Err(MessageParseError::InvalidMessage); }
            };
    
            const ARRL_CLASSES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];
            let field_day_class_string = Self::last(word).to_string();
            let field_day_class:u8 = match ARRL_CLASSES.iter().position(|s| s == &field_day_class_string) {
                Some(value) => value as u8,
                None => {
                    return Err(MessageParseError::InvalidMessage);
                }
            };
    
            return Ok((num_transmitters, num_transmitters_string, sub_type, field_day_class, field_day_class_string));
        }
        return Err(MessageParseError::InvalidMessage);
    }
    
    fn try_from_arrl_field_day_message(message_words:&[&str]) -> Result<Self, MessageParseError> {
        let mut deq = VecDeque::from_iter(message_words.iter().copied());

        let callsign1:Callsign = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let callsign2:Callsign = match Self::try_parse_callsign(&mut deq) {
            Ok(c) => c,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let ack:bool = match Self::try_parse_ack(&mut deq) {
            Ok(b) => b,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let (number_transmitters, number_transmitters_string, sub_type, field_day_class, field_day_class_string) = match Self::try_parse_transmitters_and_class(&mut deq) {
            Ok((n,ns,s,f,fs)) => (n,ns,s,f,fs),
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let section = match Self::try_parse_arrl_section(&mut deq) {
            Ok(s) => s,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let packed_bits = Self::pack_bits(&[
            (callsign1.packed_28bits.into(), 28),
            (callsign2.packed_28bits.into(), 28),
            (ack.into(), 1),
            (number_transmitters.into(), 4),
            (field_day_class.into(), 3),
            (section.packed_bits.into(), 7),
            (sub_type.into(), 3),
            (0u128, 3),
        ]);

        // Field Day    W9XYZ K1ABC R 17B EMA   c28 c28 R1 n4 k3 S7
        let ack_string = if ack {"R "} else {""};
        let packed_string = format!("{callsign1} {callsign2} {ack_string}{number_transmitters_string}{field_day_class_string} {section}");

        Ok(Message {
            packed_bits,
            display_string: packed_string,
            message_type: 0,
            message_subtype: sub_type
        })
    }

    fn try_from_telemetry_message(message_string:&str) -> Result<Self, MessageParseError> {
        if message_string.len() > 18 {
            return Err(MessageParseError::InvalidMessage);
        }

        let value = match u128::from_str_radix(message_string, 16) {
            Ok(v) => v,
            Err(_) => { return Err(MessageParseError::InvalidMessage); }
        };

        let packed_bits = Self::pack_bits(&[
            (value.into(), 71),
            (5_u128, 3),
            (0_u128, 3),
        ]);

        Ok(Message {
            packed_bits,
            display_string: message_string.to_owned(),
            message_type: 0,
            message_subtype: 5,
        })
    }

    fn try_from_free_text_string(message_string:&str) -> Result<Self, MessageParseError> {
        // the string to pack into bits must be 13 characters
        //   right align with spaces if needed
        //   trim to length if needed
        let adjusted_string:String = format!("{: >13.13}", message_string);
        let packed_string = adjusted_string.trim().to_owned(); 

        // pack the string into a u128, 71 bits.
        let f71 = match u128::from_str_custom_radix(&adjusted_string, FT8_CHAR_TABLE_FULL) {
            Ok(value) => value,
            Err(ParseRadixStringError::InvalidChar) => {
                return Err(MessageParseError::InvalidChar);
            },
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };

        // pack all the bits together
        let packed_bits = Self::pack_bits(&[
            (f71, 71),
            (0u128, 3),
            (0u128, 3)
        ]);

        Ok(Message { 
            display_string: packed_string,
            packed_bits,
            message_type: 0,
            message_subtype: 0,
        })
    }

    fn pack_bits(fields_and_widths: &[(u128, u32)]) -> u128 {
        let mut bits:u128 = 0;
        for (field, width) in fields_and_widths {
            bits = (bits << width) | *field;
        }
        bits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_parse_successfully {
        ($name:ident, $message:expr, $expected_message:expr, $expected_bits:expr, $expected_message_type:expr, $expected_message_subtype:expr) => {
            paste::item! {
                mod [< with_ $name:lower >] {
                    use super::*;

                    #[test]
                    fn packed_string_is_correct() {
                        let m = Message::try_from($message).unwrap();
                        assert_eq!(format!("{m}"), $expected_message);
                    }

                    #[test]
                    fn packed_bits_are_correct() {
                        let bits:u128 = Message::try_from($message).unwrap().into();
                        assert_eq!(bits, $expected_bits);
                    }

                    #[test]
                    fn message_type_is_correct() {
                        assert_eq!(Message::try_from($message).unwrap().message_type, $expected_message_type);   
                    }

                    #[test]
                    fn message_subtype_is_correct() {
                        assert_eq!(Message::try_from($message).unwrap().message_subtype, $expected_message_subtype);   
                    }
                }
            }
        }
    }

    mod wsjtx_tests {
        use super::*;

        // all of these tests are from wsjtx source code
        // src/wsjtx/lib/ft8/ft8_testmsg.f90
        // ran through ft8sim to determine the expected output for the tests below
        // example:
        // $ build/wsjtx-prefix/src/wsjtx-build/ft8sim "PA9XYZ 590003 IO91NP" 1500 0 0 0 1 -10
        // Decoded message: PA9XYZ 590003                           i3.n3: 0.0
        // f0: 1500.000   DT:  0.00   TxT:  12.6   SNR: -10.0  BW:50.0
        
        // Message bits: 
        // 01010101110011101110111101001101100000001101011111011010111101111011010000000
        
        // Channel symbols: 
        // 3140652362572673220023744672445005373140652010420711215646670140364610753140652
        
        //    1   0.00 1500.00  -10.0  000000_000001.wav  -10.02

        assert_parse_successfully!(wsjtx_1, "TNX BOB 73 GL", "TNX BOB 73 GL", 0b01100011111011011100111011100010101001001010111000000111111101010000000000000, 0, 0);
        assert_parse_successfully!(wsjtx_2, "K1ABC RR73; W9XYZ <KH1/KH7Z> -08", "K1ABC RR73; W9XYZ <KH1/KH7Z> -08", 0b00001001101111011110001101010000110000101001001110111000001100100101011001000, 0, 1);
        assert_parse_successfully!(wsjtx_3, "PA9XYZ 590003 IO91NP", "PA9XYZ 590003", 0b01010101110011101110111101001101100000001101011111011010111101111011010000000,0,0);
        assert_parse_successfully!(wsjtx_4, "G4ABC/P R 570007 JO22DB", "G4ABC/P R 570", 0b00110111111100010101011110000110110000011010100000000111001011111101001000000,0,0);
        assert_parse_successfully!(wsjtx_5, "K1ABC W9XYZ 6A WI", "K1ABC W9XYZ 6A WI", 0b0000100110111101111000110101_0000110000101001001110111000_0_0101_000_1001100_011_000, 0, 3);
        assert_parse_successfully!(wsjtx_6, "W9XYZ K1ABC R 17B EMA", "W9XYZ K1ABC R 17B EMA", 0b00001100001010010011101110000000100110111101111000110101100000010001011100000, 0, 4);
        assert_parse_successfully!(wsjtx_7, "123456789ABCDEF012", "123456789ABCDEF012", 0b00100100011010001010110011110001001101010111100110111101111000000010010101000, 0, 5);
        assert_parse_successfully!(wsjtx_8, "CQ K1ABC FN42", "CQ K1ABC FN42", 0b00000000000000000000000000100000010011011110111100011010100010100001100110001,1,0);   
        assert_parse_successfully!(wsjtx_9, "K1ABC W9XYZ EN37", "K1ABC W9XYZ EN37",0b00001001101111011110001101010000011000010100100111011100000010000101011001001,1,0);    
        assert_parse_successfully!(wsjtx_10, "W9XYZ K1ABC -11", "W9XYZ K1ABC -11", 0b00001100001010010011101110000000010011011110111100011010100111111010101000001,1,0);   
        assert_parse_successfully!(wsjtx_11, "K1ABC W9XYZ R-09", "K1ABC W9XYZ R-09", 0b00001001101111011110001101010000011000010100100111011100001111111010101010001,1,0); 
        assert_parse_successfully!(wsjtx_12, "W9XYZ K1ABC RRR", "W9XYZ K1ABC RRR", 0b00001100001010010011101110000000010011011110111100011010100111111010010010001,1,0);    
        assert_parse_successfully!(wsjtx_13, "K1ABC W9XYZ 73","K1ABC W9XYZ 73",0b00001001101111011110001101010000011000010100100111011100000111111010010100001,1,0);
        assert_parse_successfully!(wsjtx_14, "CQ FD K1ABC FN42", "CQ FD K1ABC FN42", 0b00000000000000000100100100010000010011011110111100011010100010100001100110001,1,0);
        assert_parse_successfully!(wsjtx_15, "CQ TEST K1ABC/R FN42", "CQ TEST K1ABC/R FN42", 0b00000000011000010101111110010000010011011110111100011010110010100001100110001,1,0);
        assert_parse_successfully!(wsjtx_16, "K1ABC/R W9XYZ EN37", "K1ABC/R W9XYZ EN37", 0b00001001101111011110001101011000011000010100100111011100000010000101011001001,1,0);
        assert_parse_successfully!(wsjtx_17, "W9XYZ K1ABC/R R FN42", "W9XYZ K1ABC/R R FN42", 0b0000110000101001001110111000_0_0000100110111101111000110101_1_1_010100001100110_001,1,0);
        assert_parse_successfully!(wsjtx_18, "K1ABC/R W9XYZ RR73", "K1ABC/R W9XYZ RR73", 0b00001001101111011110001101011000011000010100100111011100000111111001110101001,1,0);
        assert_parse_successfully!(wsjtx_19, "CQ TEST K1ABC FN42", "CQ TEST K1ABC FN42", 0b00000000011000010101111110010000010011011110111100011010100010100001100110001,1,0);
        assert_parse_successfully!(wsjtx_20, "W9XYZ <PJ4/K1ABC> -11", "W9XYZ <PJ4/K1ABC> -11", 0b00001100001010010011101110000000000110101001010110000101000111111010101000001, 1, 0);
        assert_parse_successfully!(wsjtx_21, "<PJ4/K1ABC> W9XYZ R-09", "<PJ4/K1ABC> W9XYZ R-09", 0b00000011010100101011000010100000011000010100100111011100001111111010101010001, 1, 0);
        assert_parse_successfully!(wsjtx_22, "CQ W9XYZ EN37", "CQ W9XYZ EN37", 0b00000000000000000000000000100000011000010100100111011100000010000101011001001,1,0);
        assert_parse_successfully!(wsjtx_23, "<YW18FIFA> W9XYZ -11", "<YW18FIFA> W9XYZ -11", 0b00000010101101000010101011000000011000010100100111011100000111111010101000001, 1, 0);
        assert_parse_successfully!(wsjtx_24, "W9XYZ <YW18FIFA> R-09", "W9XYZ <YW18FIFA> R-09", 0b00001100001010010011101110000000000101011010000101010110001111111010101010001, 1, 0);
        assert_parse_successfully!(wsjtx_25, "<YW18FIFA> KA1ABC", "<YW18FIFA> KA1ABC", 0b0000001010110100001010101100_0_1001010111000110010100100001_0_0_111111010010001_001,1,0);
        assert_parse_successfully!(wsjtx_26, "KA1ABC <YW18FIFA> -11", "KA1ABC <YW18FIFA> -11", 0b10010101110001100101001000010000000101011010000101010110000111111010101000001,1,0);
        assert_parse_successfully!(wsjtx_27, "<YW18FIFA> KA1ABC R-17", "<YW18FIFA> KA1ABC R-17", 0b00000010101101000010101011000100101011100011001010010000101111111010100010001,1,0);
        assert_parse_successfully!(wsjtx_28, "<YW18FIFA> KA1ABC 73", "<YW18FIFA> KA1ABC 73", 0b00000010101101000010101011000100101011100011001010010000100111111010010100001,1,0);
        assert_parse_successfully!(wsjtx_29, "CQ G4ABC/P IO91", "CQ G4ABC/P IO91", 0b00000000000000000000000000100000010010000110000010110011010011111000010011010,2,0);
        assert_parse_successfully!(wsjtx_30, "G4ABC/P PA9XYZ JO22", "G4ABC/P PA9XYZ JO22", 0b00001001000011000001011001101101101111011101011000101010000100010011010110010,2,0);
        assert_parse_successfully!(wsjtx_31, "PA9XYZ G4ABC/P RR73", "PA9XYZ G4ABC/P RR73", 0b10110111101110101100010101000000010010000110000010110011010111111001110101010,2,0);
        assert_parse_successfully!(wsjtx_32, "K1ABC W9XYZ 579 WI", "K1ABC W9XYZ 579 WI", 0b0_0000100110111101111000110101_0000110000101001001110111000_0_101_1111101110001_011, 3, 0);
        assert_parse_successfully!(wsjtx_33, "W9XYZ K1ABC R 589 MA", "W9XYZ K1ABC R 589 MA", 0b00000110000101001001110111000000010011011110111100011010111101111101010101011, 3, 0);
        assert_parse_successfully!(wsjtx_34, "K1ABC KA0DEF 559 MO", "K1ABC KA0DEF 559 MO", 0b00000100110111101111000110101100101011100001000010001110100111111101011001011, 3, 0);
        assert_parse_successfully!(wsjtx_35, "TU; KA0DEF K1ABC R 569 MA", "TU; KA0DEF K1ABC R 569 MA", 0b11001010111000010000100011101000010011011110111100011010111001111101010101011, 3, 0);
        assert_parse_successfully!(wsjtx_36, "KA1ABC G3AAA 529 0013", "KA1ABC G3AAA 529 0013", 0b01001010111000110010100100001000010010000011101000110011000000000000001101011, 3, 0);
        assert_parse_successfully!(wsjtx_37, "TU; G3AAA K1ABC R 559 MA", "TU; G3AAA K1ABC R 559 MA", 0b10000100100000111010001100110000010011011110111100011010110111111101010101011, 3, 0);
        assert_parse_successfully!(wsjtx_38, "CQ KH1/KH7Z", "CQ KH1/KH7Z", 0b00110010011000000000000000001000111100000110100011001110110000001001000001100,4,0);
        assert_parse_successfully!(wsjtx_39, "CQ PJ4/K1ABC", "CQ PJ4/K1ABC", 0b01010110101100000000000110100011101000110001000111001010101000000000010001100,4,0);
        assert_parse_successfully!(wsjtx_40, "PJ4/K1ABC <W9XYZ>", "PJ4/K1ABC <W9XYZ>", 0b11110011000100000000000110100011101000110001000111001010101000000000011000100,4,0);
        assert_parse_successfully!(wsjtx_41, "<W9XYZ> PJ4/K1ABC RRR", "<W9XYZ> PJ4/K1ABC RRR", 0b11110011000100000000000110100011101000110001000111001010101000000000010010100,4,0);
        assert_parse_successfully!(wsjtx_42, "PJ4/K1ABC <W9XYZ> 73", "PJ4/K1ABC <W9XYZ> 73", 0b11110011000100000000000110100011101000110001000111001010101000000000011110100,4,0);
        assert_parse_successfully!(wsjtx_43, "<W9XYZ> YW18FIFA", "<W9XYZ> YW18FIFA", 0b11110011000100000000000000001110111011100011100111111010101100001001110000100,4,0);
        assert_parse_successfully!(wsjtx_44, "YW18FIFA <W9XYZ> RRR", "YW18FIFA <W9XYZ> RRR", 0b11110011000100000000000000001110111011100011100111111010101100001001111010100,4,0);
        assert_parse_successfully!(wsjtx_45, "<W9XYZ> YW18FIFA 73", "<W9XYZ> YW18FIFA 73", 0b11110011000100000000000000001110111011100011100111111010101100001001110110100,4,0);
        assert_parse_successfully!(wsjtx_46, "CQ YW18FIFA", "CQ YW18FIFA", 0b00101111000100000000000000001110111011100011100111111010101100001001110001100,4,0);
        assert_parse_successfully!(wsjtx_47, "<KA1ABC> YW18FIFA RR73", "<KA1ABC> YW18FIFA RR73", 0b00101101001100000000000000001110111011100011100111111010101100001001110100100,4,0);
    }

}