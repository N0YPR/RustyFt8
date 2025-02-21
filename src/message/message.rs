use std::collections::VecDeque;
use std::fmt::Display;

use snafu::Snafu;

use super::arrl_section::ArrlSection;
use super::callsign::Callsign;
use super::constants::*;
use super::checksum::checksum;
use super::radix:: {FromStrCustomRadix, ParseRadixStringError};
use super::report::Report;
use super::serial_number_or_state_or_province::SerialNumberOrStateOrProvince;

#[derive(Debug)]
pub struct Message {
    packed_bits: u128,
    checksum: u16,
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
    pub fn bits(&self) -> u128 {
        self.packed_bits
    }

    pub fn checksum(&self) -> u16 {
        self.checksum
    }

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
            checksum: checksum(packed_bits),
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
            checksum: checksum(packed_bits),
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
            checksum: checksum(packed_bits),
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
            checksum: checksum(packed_bits),
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
            checksum: checksum(packed_bits),
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
            checksum: checksum(packed_bits),
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
            checksum: checksum(packed_bits),
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
        ($name:ident, $message:expr, $expected_message:expr, $channel_symbols_str:expr) => {
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
                        assert_eq!(bits, extract_message_bits_from_symbols_str($channel_symbols_str));
                    }

                    #[test]
                    fn message_type_is_correct() {
                        let bits:u128 = Message::try_from($message).unwrap().into();
                        assert_eq!(bits & 0b111u128, extract_message_bits_from_symbols_str($channel_symbols_str) & 0b111u128);   
                    }

                    #[test]
                    fn message_subtype_is_correct_if_applicable() {
                        let expected_bits = extract_message_bits_from_symbols_str($channel_symbols_str);
                        let expected_type = expected_bits & 0b111u128;

                        if expected_type == 0b000 {
                            let expected_subtype = (expected_bits >> 3) & 0b111u128;

                            let bits:u128 = Message::try_from($message).unwrap().into();
                            let actual_subtype = (bits >> 3) & 0b111u128;

                            assert_eq!(expected_subtype, actual_subtype);
                        }

                    }

                }
            }
        }
    }

    mod wsjtx_tests {
        use crate::encode::gray::{GrayCode, FT8_GRAY_CODE};

        use super::*;

        fn extract_gray_decoded_message_symbols_from_symbols_str(symbols_str: &str) -> Vec<u8> {
            if symbols_str.len() != 79 {
                panic!("Input string must be 79 characters long.");
            }

            let symbols_str_without_costas = format!("{}{}", &symbols_str[7..36], &symbols_str[43..72]);

            let symbols: Vec<u8> = symbols_str_without_costas.chars()
                .map(|c| c.to_digit(8) // Parse as octal digit (0-7)
                    .expect("Input contains invalid characters") as u8)
                .collect();

            let gray = GrayCode::new(&FT8_GRAY_CODE);
            let gray_decoded_symbols = gray.decode(&symbols);

            gray_decoded_symbols
        }

        fn extract_message_bits_from_symbols_str(symbols_str: &str) -> u128 {
            let symbols = extract_gray_decoded_message_symbols_from_symbols_str(symbols_str);
            
            // want the 0..77 bits
            let mut bits: u128 = 0;
            for &symbol in symbols.iter().take(26) {
                let lowest_three_bits = symbol & 0b111;
                bits = bits << 3;
                bits |= lowest_three_bits as u128;
            }
            bits = (bits >> 1) & ((1_u128 << 77) - 1);
            
            bits
        }

        fn extract_crc_bits_from_symbols_str(symbols_str: &str) -> u16 {
            let symbols = extract_gray_decoded_message_symbols_from_symbols_str(symbols_str);

            let mut crc_bits: u16 = 0;
            for &symbol in symbols.iter().skip(25).take(6) {
                let lowest_three_bits = symbol & 0b111;
                crc_bits = crc_bits << 3;
                crc_bits |= lowest_three_bits as u16;
            }
            crc_bits = (crc_bits >> 2) & 0b11111111111111;
            
            crc_bits
        }

        // all of these tests are from wsjtx source code
        // src/wsjtx/lib/ft8/ft8_testmsg.f90
        // ran through ft8sim to determine the expected output for the tests below
        // example:
        // $ build/wsjtx-prefix/src/wsjtx-build/ft8sim "TNX BOB 73 GL" 1500 0 0 0 1 -10
        //   Decoded message: TNX BOB 73 GL                           i3.n3: 0.0
        //   f0: 1500.000   DT:  0.00   TxT:  12.6   SNR: -10.0  BW:50.0
        
        //   Message bits: 
        //   01100011111011011100111011100010101001001010111000000111111101010000000000000
        
        //   Channel symbols: 
        //   3140652207447147063336401773500017703140652646427306546072440503670130533140652
        //      1   0.00 1500.00  -10.0  000000_000001.wav   -9.99

        assert_parse_successfully!(wsjtx_1, "TNX BOB 73 GL", "TNX BOB 73 GL", "3140652207447147063336401773500017703140652646427306546072440503670130533140652");
        assert_parse_successfully!(wsjtx_2, "K1ABC RR73; W9XYZ <KH1/KH7Z> -08", "K1ABC RR73; W9XYZ <KH1/KH7Z> -08", "3140652032247523515133264021134317153140652027407072730041362310127254663140652");
        assert_parse_successfully!(wsjtx_3, "PA9XYZ 590003 IO91NP", "PA9XYZ 590003", "3140652362572673220023744672445005373140652010420711215646670140364610753140652");
        assert_parse_successfully!(wsjtx_4, "G4ABC/P R 570007 JO22DB", "G4ABC/P R 570", "3140652167706375165046001437733003363140652220745304647271234314310031673140652");
        assert_parse_successfully!(wsjtx_5, "K1ABC W9XYZ 6A WI", "K1ABC W9XYZ 6A WI", "3140652032247523515133264035320405303140652101020166700026554505077720623140652");
        assert_parse_successfully!(wsjtx_6, "W9XYZ K1ABC R 17B EMA", "W9XYZ K1ABC R 17B EMA", "3140652020355725011672416200537013033140652330677001403444125317721563223140652");
        assert_parse_successfully!(wsjtx_7, "123456789ABCDEF012", "123456789ABCDEF012", "3140652110453657532367167240056304313140652620633153646703256576437647343140652");
        assert_parse_successfully!(wsjtx_8, "CQ K1ABC FN42", "CQ K1ABC FN42", "3140652000000001005476704606021533433140652736011047517007334745455133543140652");
        assert_parse_successfully!(wsjtx_9, "K1ABC W9XYZ EN37", "K1ABC W9XYZ EN37", "3140652032247523504061147005134325373140652464557561564770300376175462233140652");
        assert_parse_successfully!(wsjtx_10, "W9XYZ K1ABC -11", "W9XYZ K1ABC -11", "3140652020355725005476704617463024063140652536316515751700077044377507213140652");
        assert_parse_successfully!(wsjtx_11, "K1ABC W9XYZ R-09", "K1ABC W9XYZ R-09", "3140652032247523504061147027463527033140652323406130213743267634453040613140652");
        assert_parse_successfully!(wsjtx_12, "W9XYZ K1ABC RRR", "W9XYZ K1ABC RRR", "3140652020355725005476704617455530313140652564305535161117524523127753273140652");
        assert_parse_successfully!(wsjtx_13, "K1ABC W9XYZ 73", "K1ABC W9XYZ 73", "3140652032247523504061147017456023753140652176074113361533126044715626273140652");
        assert_parse_successfully!(wsjtx_14, "K1ABC W9XYZ RR73", "K1ABC W9XYZ RR73", "3140652032247523504061147017426332613140652071301161600346511151226424023140652");
        assert_parse_successfully!(wsjtx_15, "CQ FD K1ABC FN42", "CQ FD K1ABC FN42", "3140652000001110505476704606021533743140652551744705346540117264367236423140652");
        assert_parse_successfully!(wsjtx_16, "CQ TEST K1ABC/R FN42", "CQ TEST K1ABC/R FN42", "3140652000406275505476704656021522243140652712131455071561243646177737743140652");
        assert_parse_successfully!(wsjtx_17, "K1ABC/R W9XYZ EN37", "K1ABC/R W9XYZ EN37", "3140652032247523404061147005134332153140652623707512241501513760247527103140652");
        assert_parse_successfully!(wsjtx_18, "W9XYZ K1ABC/R R FN42", "W9XYZ K1ABC/R R FN42", "3140652020355725005476704646021534063140652447233323457764637506512367623140652");
        assert_parse_successfully!(wsjtx_19, "K1ABC/R W9XYZ RR73", "K1ABC/R W9XYZ RR73", "3140652032247523404061147017426325433140652216151112327115302545134563313140652");
        assert_parse_successfully!(wsjtx_20, "CQ TEST K1ABC FN42", "CQ TEST K1ABC FN42", "3140652000406275505476704606021520133140652212501560611771401652231035343140652");
        assert_parse_successfully!(wsjtx_21, "W9XYZ <PJ4/K1ABC> -11", "W9XYZ <PJ4/K1ABC> -11", "3140652020355725001633651317463025333140652721702305367726741577047037163140652");
        assert_parse_successfully!(wsjtx_22, "<PJ4/K1ABC> W9XYZ R-09", "<PJ4/K1ABC> W9XYZ R-09", "3140652004613406004061147027463523403140652700266426703075361110173346223140652");
        assert_parse_successfully!(wsjtx_23, "CQ W9XYZ EN37", "CQ W9XYZ EN37", "3140652000000001004061147005134327023140652527476570561660640101346156613140652");
        assert_parse_successfully!(wsjtx_24, "<YW18FIFA> W9XYZ -11", "<YW18FIFA> W9XYZ -11", "3140652006230634004061147017463025173140652301501240633504530456107701703140652");
        assert_parse_successfully!(wsjtx_25, "W9XYZ <YW18FIFA> R-09", "W9XYZ <YW18FIFA> R-09", "3140652020355725001345136527463535673140652666243061260136572121271345123140652");
        assert_parse_successfully!(wsjtx_26, "<YW18FIFA> KA1ABC", "<YW18FIFA> KA1ABC", "3140652006230634113704355117455325073140652112346203553211534271220352553140652");
        assert_parse_successfully!(wsjtx_27, "KA1ABC <YW18FIFA> -11", "KA1ABC <YW18FIFA> -11", "3140652562521330501345136517463035563140652745336500352710660271112473543140652");
        assert_parse_successfully!(wsjtx_28, "<YW18FIFA> KA1ABC R-17", "<YW18FIFA> KA1ABC R-17", "3140652006230634113704355127460530343140652746043101745421563500056465063140652");
        assert_parse_successfully!(wsjtx_29, "<YW18FIFA> KA1ABC 73", "<YW18FIFA> KA1ABC 73", "3140652006230634113704355117456020263140652673662145153445157102313527513140652");
        assert_parse_successfully!(wsjtx_30, "CQ G4ABC/P IO91", "CQ G4ABC/P IO91", "3140652000000001005515065457405456273140652311753555773213266103254602113140652");
        assert_parse_successfully!(wsjtx_31, "G4ABC/P PA9XYZ JO22", "G4ABC/P PA9XYZ JO22", "3140652033040342222473413510546556673140652125365204412473533331244335523140652");
        assert_parse_successfully!(wsjtx_32, "PA9XYZ G4ABC/P RR73", "PA9XYZ G4ABC/P RR73", "3140652667262063005515065467426366703140652155174750577504502006433672343140652");
        assert_parse_successfully!(wsjtx_33, "K1ABC W9XYZ 579 WI", "K1ABC W9XYZ 579 WI", "3140652011672416304061147037725347523140652306512463403404071636453510363140652");
        assert_parse_successfully!(wsjtx_34, "W9XYZ K1ABC R 589 MA", "W9XYZ K1ABC R 589 MA", "3140652015133264005476704672736370703140652556231412670171422210666331723140652");
        assert_parse_successfully!(wsjtx_35, "K1ABC KA0DEF 559 MO", "K1ABC KA0DEF 559 MO", "3140652011672416213703052617734344213140652530733115357714754126135471623140652");
        assert_parse_successfully!(wsjtx_36, "TU; KA0DEF K1ABC R 569 MA", "TU; KA0DEF K1ABC R 569 MA", "3140652436405107305476704642736345103140652330752307172673211532446754253140652");
        assert_parse_successfully!(wsjtx_37, "KA1ABC G3AAA 529 0013", "KA1ABC G3AAA 529 0013", "3140652336415610305507315400002343163140652702747234356765754244623420063140652");
        assert_parse_successfully!(wsjtx_38, "TU; G3AAA K1ABC R 559 MA", "TU; G3AAA K1ABC R 559 MA", "3140652511014521505476704667736374673140652147443157301235307742101161613140652");
        assert_parse_successfully!(wsjtx_39, "CQ KH1/KH7Z", "CQ KH1/KH7Z", "3140652155400000317016042650330214403140652246332541464425542473300211553140652");
        assert_parse_successfully!(wsjtx_40, "CQ PJ4/K1ABC", "CQ PJ4/K1ABC", "3140652366200016073153143630005210413140652661416746414647456323744275423140652");
        assert_parse_successfully!(wsjtx_41, "PJ4/K1ABC <W9XYZ>", "PJ4/K1ABC <W9XYZ>", "3140652754100016073153143630004104403140652260770176145261322551452103013140652");
        assert_parse_successfully!(wsjtx_42, "<W9XYZ> PJ4/K1ABC RRR", "<W9XYZ> PJ4/K1ABC RRR", "3140652754100016073153143630005614063140652361206660067077171261117407013140652");
        assert_parse_successfully!(wsjtx_43, "PJ4/K1ABC <W9XYZ> 73", "PJ4/K1ABC <W9XYZ> 73", "3140652754100016073153143630007611403140652310172166217632341002174415723140652");
        assert_parse_successfully!(wsjtx_44, "<W9XYZ> YW18FIFA", "<W9XYZ> YW18FIFA", "3140652754100000264707174620325114443140652126305246567642322733274461643140652");
        assert_parse_successfully!(wsjtx_45, "YW18FIFA <W9XYZ> RRR", "YW18FIFA <W9XYZ> RRR", "3140652754100000264707174620324604023140652025471750645456171023533165643140652");
        assert_parse_successfully!(wsjtx_46, "<W9XYZ> YW18FIFA 73", "<W9XYZ> YW18FIFA 73", "3140652754100000264707174620326601443140652076507256435211341240552157353140652");
        assert_parse_successfully!(wsjtx_47, "CQ YW18FIFA", "CQ YW18FIFA", "3140652124100000264707174620325205033140652432356364551041722633453063573140652");
        assert_parse_successfully!(wsjtx_48, "<KA1ABC> YW18FIFA RR73", "<KA1ABC> YW18FIFA RR73", "3140652123200000264707174620326107553140652730410160050034134266602045713140652");
    }

}