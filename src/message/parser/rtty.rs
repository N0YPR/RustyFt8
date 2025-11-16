//! RTTY Roundup Contest Message Parsing
//!
//! This module handles parsing of RTTY Roundup contest messages, which use the format:
//! "[TU;] CALL1 CALL2 [R] <5X9> <state|serial>"
//!
//! RTTY messages use a specific RST format (5X9 where X is 2-9) and include either
//! a US state/province code or a 4-digit serial number. This is a Type 1 message variant.

use crate::message::types::MessageVariant;
use super::validators::validate_callsign_basic;

/// Parse RTTY Roundup message (4+ words)
///
/// Format: "[TU;] CALL1 CALL2 [R] <5X9> <state|serial>"
/// Example: "K1ABC W9XYZ 579 WI" or "TU; K1ABC KA0DEF 559 MO"
///
/// Where:
/// - TU;: Optional "Thank you" prefix
/// - 5X9: RST report where X is 2-9 (middle digit - 2 encodes the value)
/// - state: 2-3 letter US state/province code
/// - serial: 4-digit serial number
pub(super) fn parse_rtty_message(parts: &[&str]) -> Result<MessageVariant, String> {
    let mut idx = 0;
    let tu = if parts[0] == "TU;" {
        idx = 1;
        true
    } else {
        false
    };

    if idx + 3 >= parts.len() {
        return Err("Not enough parts for RTTY message".into());
    }

    let call1 = parts[idx].to_uppercase();
    let call2 = parts[idx + 1].to_uppercase();
    let has_r = parts[idx + 2] == "R";
    let exchange_idx = if has_r { idx + 3 } else { idx + 2 };
    let state_idx = exchange_idx + 1;

    if state_idx >= parts.len() {
        return Err("Missing state/exchange in RTTY message".into());
    }

    let exchange_str = parts[exchange_idx];
    let state_str = parts[state_idx].to_uppercase();

    // Check if exchange matches RTTY format: 5X9 where X is 2-9
    if exchange_str.len() == 3 &&
       exchange_str.starts_with('5') &&
       exchange_str.ends_with('9') {
        let middle_char = exchange_str.chars().nth(1).unwrap();
        if middle_char >= '2' && middle_char <= '9' {
            let is_state = state_str.len() >= 2 && state_str.len() <= 3 &&
                          state_str.chars().all(|c| c.is_ascii_alphabetic());
            let is_serial = state_str.len() == 4 &&
                           state_str.chars().all(|c| c.is_ascii_digit());

            if is_state || is_serial {
                validate_callsign_basic(&call1)?;
                validate_callsign_basic(&call2)?;

                return Ok(MessageVariant::RttyRoundup {
                    tu,
                    call1,
                    call2,
                    r_flag: has_r,
                    rst: middle_char as u8 - b'0' - 2,
                    exchange: state_str,
                });
            }
        }
    }

    Err("Not a valid RTTY message".into())
}
