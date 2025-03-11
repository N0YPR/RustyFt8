use std::collections::VecDeque;

use super::{arrl_section::ArrlSection, callsign::Callsign, message_parse_error::MessageParseError, report::Report, serial_number_or_state_or_province::SerialNumberOrStateOrProvince};

pub fn try_parse_standard_report(deq: &mut VecDeque<&str>) -> Result<Report, MessageParseError> {
    if let Some(word) = deq.pop_front() {
        let word_to_parse: String;
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
            Ok(c) => return Ok(c),
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
    } else {
        match Report::try_from_report_str("") {
            Ok(r) => return Ok(r),
            Err(_) => {
                return Err(MessageParseError::InvalidMessage);
            }
        };
    }
}

pub fn try_parse_callsign(deq: &mut VecDeque<&str>) -> Result<Callsign, MessageParseError> {
    if let Some(word) = deq.pop_front() {
        if word == "CQ" {
            if let Some(next_word) = deq.pop_front() {
                if let Ok(callsign) =
                    Callsign::from_callsign_str(&format!("{} {}", word, next_word))
                {
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

pub fn try_parse_thank_you(deq: &mut VecDeque<&str>) -> Result<bool, MessageParseError> {
    match deq.pop_front() {
        Some(word) => {
            let tu = word == "TU;";
            if !tu {
                deq.push_front(word);
            }
            return Ok(tu);
        }
        None => {
            return Err(MessageParseError::InvalidMessage);
        }
    }
}

pub fn try_parse_rtty_report(deq: &mut VecDeque<&str>) -> Result<(u32, String), MessageParseError> {
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

pub fn try_parse_rtty_serial_or_state(
    deq: &mut VecDeque<&str>,
) -> Result<SerialNumberOrStateOrProvince, MessageParseError> {
    if let Some(word) = deq.pop_front() {
        if let Ok(s) = SerialNumberOrStateOrProvince::try_from_string(word) {
            return Ok(s);
        }
    }
    return Err(MessageParseError::InvalidMessage);
}

pub fn try_parse_ack(deq: &mut VecDeque<&str>) -> Result<bool, MessageParseError> {
    if let Some(word) = deq.pop_front() {
        let ack = word == "R";
        if !ack {
            deq.push_front(word);
        }
        return Ok(ack);
    }
    return Err(MessageParseError::InvalidMessage);
}

pub fn try_parse_arrl_section(deq: &mut VecDeque<&str>) -> Result<ArrlSection, MessageParseError> {
    if let Some(word) = deq.pop_front() {
        return match ArrlSection::try_from_string(word) {
            Ok(s) => Ok(s),
            Err(_) => Err(MessageParseError::InvalidMessage),
        };
    }
    return Err(MessageParseError::InvalidMessage);
}

pub fn try_parse_transmitters_and_class(
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