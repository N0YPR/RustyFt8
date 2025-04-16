use std::fmt::Display;

use message_parse_error::MessageParseError;

mod arrl_field_day_message;
mod arrl_section;
mod callsign;
mod dxpedition_message;
mod free_text_message;
mod non_standard_message;
mod parse;
mod radix;
mod report;
mod rtty_roundup_message;
mod serial_number_or_state_or_province;
mod standard_or_eu_vhf_message;
mod telemetry_message;

#[cfg(test)]
mod tests;

pub mod message_parse_error;

#[derive(Debug)]
pub struct Message {
    pub message: u128,
    pub display_string: String,
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_string)
    }
}

impl TryFrom<&str> for Message {
    type Error = MessageParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(MessageParseError::EmptyString);
        }

        // standard or EU VHF
        if let Ok(message) = standard_or_eu_vhf_message::try_from_string(value) {
            return Ok(message);
        }

        // non-standard
        if let Ok(message) = non_standard_message::try_from_string(value) {
            return Ok(message);
        }

        // RTTY Roundup
        if let Ok(message) = rtty_roundup_message::try_from_string(value) {
            return Ok(message);
        }

        // DXpedition
        if let Ok(message) = dxpedition_message::try_from_string(value) {
            return Ok(message);
        }

        // ARRL Field Day
        if let Ok(message) = arrl_field_day_message::try_from_string(value) {
            return Ok(message);
        }

        // Telemetry
        if let Ok(message) = telemetry_message::try_from_string(value) {
            return Ok(message);
        }

        // free text message
        if let Ok(message) = free_text_message::try_from_string(value) {
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

impl TryFrom<u128> for Message {
    type Error = MessageParseError;

    fn try_from(value: u128) -> Result<Self, Self::Error> {
        // standard or EU VHF
        if let Ok(message) = standard_or_eu_vhf_message::try_from_u128(value) {
            return Ok(message);
        }

        // non-standard
        if let Ok(message) = non_standard_message::try_from_u128(value) {
            return Ok(message);
        }

        // RTTY Roundup
        if let Ok(message) = rtty_roundup_message::try_from_u128(value) {
            return Ok(message);
        }

        // DXpedition
        if let Ok(message) = dxpedition_message::try_from_u128(value) {
            return Ok(message);
        }

        // ARRL Field Day
        if let Ok(message) = arrl_field_day_message::try_from_u128(value) {
            return Ok(message);
        }

        // Telemetry
        if let Ok(message) = telemetry_message::try_from_u128(value) {
            return Ok(message);
        }
        
        // free text message
        if let Ok(message) = free_text_message::try_from_u128(value) {
            return Ok(message);
        }

        // unable to parse
        return Err(MessageParseError::InvalidMessage);
    }
}