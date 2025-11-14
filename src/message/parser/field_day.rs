use alloc::string::{String, ToString};
use alloc::format;
use crate::message::types::MessageVariant;
use crate::message::lookup_tables::arrl_section_to_index;
use super::validators::validate_callsign_basic;

/// Parse Field Day message (4+ words)
///
/// Format: "CALL1 CALL2 [R] <ntx><class> <section>"
/// Example: "K1ABC W9XYZ 6A WI" or "W9XYZ K1ABC R 17B EMA"
///
/// Where:
/// - ntx: Number of transmitters (1-32)
/// - class: Letter A-F
/// - section: ARRL section code (2-3 letters)
pub(super) fn parse_field_day_message(parts: &[&str]) -> Result<MessageVariant, String> {
    let has_r = parts.len() >= 5 && parts[2] == "R";
    let class_idx = if has_r { 3 } else { 2 };
    let section_idx = class_idx + 1;

    if section_idx >= parts.len() {
        return Err("Not enough parts for Field Day message".into());
    }

    let call1 = parts[0].to_uppercase();
    let call2 = parts[1].to_uppercase();
    let class_str = parts[class_idx];
    let section_str = parts[section_idx].to_uppercase();

    let class_len = class_str.len();
    if class_len < 2 {
        return Err("Invalid Field Day class".into());
    }

    let (num_str, letter_str) = class_str.split_at(class_len - 1);
    let letter_char = letter_str.chars().next().unwrap().to_ascii_uppercase();

    if letter_char >= 'A' && letter_char <= 'F' {
        if let Ok(ntx) = num_str.parse::<u8>() {
            if ntx >= 1 && ntx <= 32 {
                if let Some(isec) = arrl_section_to_index(&section_str) {
                    validate_callsign_basic(&call1)?;
                    validate_callsign_basic(&call2)?;

                    let (n3, intx) = if ntx <= 16 {
                        (3, ntx - 1)
                    } else {
                        (4, ntx - 17)
                    };

                    let nclass = (letter_char as u8) - b'A';

                    return Ok(MessageVariant::FieldDay {
                        call1,
                        call2,
                        r_flag: has_r,
                        intx,
                        nclass,
                        isec,
                        n3,
                    });
                }
            }
        }
    }

    Err("Not a valid Field Day message".into())
}
