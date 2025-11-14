use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;
use crate::message::types::MessageVariant;
use crate::message::validation::{validate_callsign_basic, validate_grid_basic, is_nonstandard_callsign};
use crate::message::lookup_tables::arrl_section_to_index;

/// Parse text message into internal MessageVariant representation
pub fn parse_message_variant(text: &str) -> Result<MessageVariant, String> {
    let trimmed = text.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    
    // Try to parse as Standard CQ message: "CQ [CALLSIGN] [GRID]" or "CQ [WORD] [CALLSIGN] [GRID]"
    if parts.len() == 3 && parts[0].eq_ignore_ascii_case("CQ") {
        let callsign = parts[1].to_uppercase();
        let grid = parts[2].to_uppercase();
        
        // Check for /R or /P suffix on callsign
        let (base_callsign, has_suffix, is_p_suffix) = if callsign.ends_with("/R") {
            (callsign.strip_suffix("/R").unwrap().to_string(), true, false)
        } else if callsign.ends_with("/P") {
            (callsign.strip_suffix("/P").unwrap().to_string(), true, true)
        } else {
            (callsign, false, false)
        };
        
        // Validate using existing functions
        validate_callsign_basic(&base_callsign)?;
        validate_grid_basic(&grid)?;
        
        // Use Type 2 for /P suffixes, Type 1 (Standard) for /R or no suffix
        if is_p_suffix {
            return Ok(MessageVariant::EuVhfContestType2 {
                call1: "CQ".to_string(),
                call1_suffix: false,
                call2: base_callsign,
                call2_suffix: true,
                r_flag: false,
                grid_or_report: grid,
            });
        } else {
            return Ok(MessageVariant::Standard {
                call1: "CQ".to_string(),
                call1_suffix: false,
                call2: base_callsign,
                call2_suffix: has_suffix,
                r_flag: false,
                grid_or_report: grid,
            });
        }
    }
    
    // Try to parse as Directed CQ: "CQ [WORD] [CALLSIGN] [GRID]"
    if parts.len() == 4 && parts[0].eq_ignore_ascii_case("CQ") {
        let cq_modifier = parts[1].to_uppercase();
        let callsign = parts[2].to_uppercase();
        let grid = parts[3].to_uppercase();
        
        // Check for /R or /P suffix on callsign
        let (base_callsign, has_suffix, is_p_suffix) = if callsign.ends_with("/R") {
            (callsign.strip_suffix("/R").unwrap().to_string(), true, false)
        } else if callsign.ends_with("/P") {
            (callsign.strip_suffix("/P").unwrap().to_string(), true, true)
        } else {
            (callsign, false, false)
        };
        
        // Validate using existing functions
        validate_callsign_basic(&base_callsign)?;
        validate_grid_basic(&grid)?;
        
        // Construct the directed CQ call (e.g., "CQ SOTA")
        let cq_call = format!("CQ {}", cq_modifier);
        
        // Use Type 2 for /P suffixes, Type 1 (Standard) for /R or no suffix
        if is_p_suffix {
            return Ok(MessageVariant::EuVhfContestType2 {
                call1: cq_call,
                call1_suffix: false,
                call2: base_callsign,
                call2_suffix: true,
                r_flag: false,
                grid_or_report: grid,
            });
        } else {
            return Ok(MessageVariant::Standard {
                call1: cq_call,
                call1_suffix: false,
                call2: base_callsign,
                call2_suffix: has_suffix,
                r_flag: false,
                grid_or_report: grid,
            });
        }
    }
    
    // Try 2-word messages
    if parts.len() == 2 {
        return parse_two_word_message(&parts, trimmed);
    }
    
    // Try 3-word messages
    if parts.len() == 3 {
        return parse_three_word_message(&parts, trimmed);
    }
    
    // Try 4-word messages
    if parts.len() == 4 {
        if let Ok(msg) = parse_four_word_message(&parts) {
            return Ok(msg);
        }
        // If 4-word parsing failed, try as free text
    }
    
    // Try 5-word DXpedition message
    if parts.len() == 5 && parts[1] == "RR73;" {
        return parse_dxpedition_message(&parts);
    }
    
    // Try RTTY Roundup (4+ words)
    if parts.len() >= 4 {
        if let Ok(msg) = parse_rtty_message(&parts) {
            return Ok(msg);
        }
    }
    
    // Try Field Day (4+ words)
    if parts.len() >= 4 {
        if let Ok(msg) = parse_field_day_message(&parts) {
            return Ok(msg);
        }
    }
    
    // Try Telemetry (hex string)
    if trimmed.len() <= 18 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return parse_telemetry_message(trimmed);
    }
    
    // Default to free text
    parse_free_text_message(trimmed)
}

fn parse_two_word_message(parts: &[&str], trimmed: &str) -> Result<MessageVariant, String> {
    let call1 = parts[0].to_uppercase();
    let call2 = parts[1].to_uppercase();
    
    // Special case: "CQ NONSTANDARD"
    if call1 == "CQ" && is_nonstandard_callsign(&call2) {
        return Ok(MessageVariant::NonStandardCall {
            text: trimmed.to_string(),
        });
    }
    
    // Special case: "COMPOUND <HASH>"
    if call1.contains('/') && call2.starts_with('<') && call2.ends_with('>') {
        let is_compound = !call1.ends_with("/P") && !call1.ends_with("/R");
        if is_compound {
            return Ok(MessageVariant::NonStandardCall {
                text: trimmed.to_string(),
            });
        }
    }
    
    // Special case: "<HASH> COMPOUND"
    if call1.starts_with('<') && call1.ends_with('>') && call2.contains('/') {
        let is_compound = !call2.ends_with("/P") && !call2.ends_with("/R");
        if is_compound {
            return Ok(MessageVariant::NonStandardCall {
                text: trimmed.to_string(),
            });
        }
    }
    
    // Special case: "<HASH> NONSTANDARD" or "NONSTANDARD <HASH>"
    if (call1.starts_with('<') && call1.ends_with('>') && is_nonstandard_callsign(&call2)) ||
       (call2.starts_with('<') && call2.ends_with('>') && is_nonstandard_callsign(&call1)) {
        return Ok(MessageVariant::NonStandardCall {
            text: trimmed.to_string(),
        });
    }
    
    // Standard 2-word message
    let (base_call1, has_suffix1) = strip_suffix(&call1);
    let (base_call2, has_suffix2) = strip_suffix(&call2);
    
    validate_callsign_basic(&base_call1)?;
    validate_callsign_basic(&base_call2)?;
    
    Ok(MessageVariant::Standard {
        call1: base_call1,
        call1_suffix: has_suffix1,
        call2: base_call2,
        call2_suffix: has_suffix2,
        r_flag: false,
        grid_or_report: String::new(),
    })
}

fn parse_three_word_message(parts: &[&str], trimmed: &str) -> Result<MessageVariant, String> {
    let call1 = parts[0].to_uppercase();
    let call2 = parts[1].to_uppercase();
    let grid_or_report = parts[2].to_uppercase();
    
    // Check for NonStandardCall patterns with acknowledgments
    let first_is_hash = call1.starts_with('<') && call1.ends_with('>');
    let second_is_hash = call2.starts_with('<') && call2.ends_with('>');
    let first_is_nonstandard = is_nonstandard_callsign(&call1);
    let second_is_nonstandard = is_nonstandard_callsign(&call2);
    let is_ack = grid_or_report == "RRR" || grid_or_report == "RR73" || grid_or_report == "73";
    
    if is_ack && ((first_is_hash && second_is_nonstandard) || (first_is_nonstandard && second_is_hash)) {
        return Ok(MessageVariant::NonStandardCall {
            text: trimmed.to_string(),
        });
    }
    
    // Parse suffixes
    let (base_call1, has_suffix1, is_p_suffix1) = parse_suffix(&call1);
    let (base_call2, has_suffix2, is_p_suffix2) = parse_suffix(&call2);
    
    // Check for R prefix in grid_or_report
    let (r_flag, final_grid_or_report) = if grid_or_report.starts_with("R-") || grid_or_report.starts_with("R+") {
        (true, grid_or_report[1..].to_string())
    } else {
        (false, grid_or_report)
    };
    
    validate_callsign_basic(&base_call1)?;
    validate_callsign_basic(&base_call2)?;
    validate_grid_basic(&final_grid_or_report)?;
    
    if is_p_suffix1 || is_p_suffix2 {
        Ok(MessageVariant::EuVhfContestType2 {
            call1: base_call1,
            call1_suffix: is_p_suffix1,
            call2: base_call2,
            call2_suffix: is_p_suffix2,
            r_flag,
            grid_or_report: final_grid_or_report,
        })
    } else {
        Ok(MessageVariant::Standard {
            call1: base_call1,
            call1_suffix: has_suffix1,
            call2: base_call2,
            call2_suffix: has_suffix2,
            r_flag,
            grid_or_report: final_grid_or_report,
        })
    }
}

fn parse_four_word_message(parts: &[&str]) -> Result<MessageVariant, String> {
    if parts[2].eq_ignore_ascii_case("R") {
        let call1 = parts[0].to_uppercase();
        let call2 = parts[1].to_uppercase();
        let grid_or_report = parts[3].to_uppercase();
        
        let (base_call1, has_suffix1, is_p_suffix1) = parse_suffix(&call1);
        let (base_call2, has_suffix2, is_p_suffix2) = parse_suffix(&call2);
        
        validate_callsign_basic(&base_call1)?;
        validate_callsign_basic(&base_call2)?;
        validate_grid_basic(&grid_or_report)?;
        
        if is_p_suffix1 || is_p_suffix2 {
            return Ok(MessageVariant::EuVhfContestType2 {
                call1: base_call1,
                call1_suffix: is_p_suffix1,
                call2: base_call2,
                call2_suffix: is_p_suffix2,
                r_flag: true,
                grid_or_report,
            });
        } else {
            return Ok(MessageVariant::Standard {
                call1: base_call1,
                call1_suffix: has_suffix1,
                call2: base_call2,
                call2_suffix: has_suffix2,
                r_flag: true,
                grid_or_report,
            });
        }
    }
    
    Err("Failed to parse 4-word message".into())
}

fn parse_dxpedition_message(parts: &[&str]) -> Result<MessageVariant, String> {
    let call1 = parts[0].to_uppercase();
    let call2 = parts[2].to_uppercase();
    let hash_call_with_brackets = parts[3];
    let report_str = parts[4];
    
    if !hash_call_with_brackets.starts_with('<') || !hash_call_with_brackets.ends_with('>') {
        return Err(format!("DXpedition mode requires angle brackets: '{}'", hash_call_with_brackets));
    }
    
    let hash_call = hash_call_with_brackets[1..hash_call_with_brackets.len()-1].to_string();
    let report: i8 = report_str.parse()
        .map_err(|_| format!("Invalid signal report: '{}'", report_str))?;
    
    if report < -30 || report > 32 {
        return Err(format!("Signal report out of range (-30 to +32): {}", report));
    }
    
    validate_callsign_basic(&call1)?;
    validate_callsign_basic(&call2)?;
    
    Ok(MessageVariant::DXpedition {
        call1,
        call2,
        hash_call,
        report,
    })
}

fn parse_rtty_message(parts: &[&str]) -> Result<MessageVariant, String> {
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

fn parse_field_day_message(parts: &[&str]) -> Result<MessageVariant, String> {
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

fn parse_telemetry_message(trimmed: &str) -> Result<MessageVariant, String> {
    let hex_string = format!("{:0>18}", trimmed.to_uppercase());
    
    let ntel1 = u32::from_str_radix(&hex_string[0..6], 16)
        .map_err(|_| format!("Invalid hex in telemetry: '{}'", hex_string))?;
    let _ntel2 = u32::from_str_radix(&hex_string[6..12], 16)
        .map_err(|_| format!("Invalid hex in telemetry: '{}'", hex_string))?;
    let _ntel3 = u32::from_str_radix(&hex_string[12..18], 16)
        .map_err(|_| format!("Invalid hex in telemetry: '{}'", hex_string))?;
    
    if ntel1 >= 0x800000 {
        return Err(format!("Telemetry first 6 hex digits exceed 23 bits: 0x{:06X}", ntel1));
    }
    
    Ok(MessageVariant::Telemetry {
        hex_string,
    })
}

fn parse_free_text_message(trimmed: &str) -> Result<MessageVariant, String> {
    if trimmed.len() > 13 {
        return Err(format!("Message too long for free text (max 13 chars): '{}'", trimmed));
    }
    
    const CHARSET: &str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";
    let upper_text = trimmed.to_uppercase();
    for ch in upper_text.chars() {
        if !CHARSET.contains(ch) {
            return Err(format!("Invalid character '{}' for free text message", ch));
        }
    }
    
    Ok(MessageVariant::FreeText {
        text: upper_text,
    })
}

// Helper functions

fn strip_suffix(callsign: &str) -> (String, bool) {
    if callsign.ends_with("/R") || callsign.ends_with("/P") {
        (callsign[..callsign.len()-2].to_string(), true)
    } else {
        (callsign.to_string(), false)
    }
}

fn parse_suffix(callsign: &str) -> (String, bool, bool) {
    if callsign.ends_with("/R") {
        (callsign.strip_suffix("/R").unwrap().to_string(), true, false)
    } else if callsign.ends_with("/P") {
        (callsign.strip_suffix("/P").unwrap().to_string(), true, true)
    } else {
        (callsign.to_string(), false, false)
    }
}
