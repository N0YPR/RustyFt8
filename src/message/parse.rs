use std::collections::VecDeque;

use super::{callsign::Callsign, message_parse_error::MessageParseError, report::Report};

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


