use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;

/// Basic validation for callsigns
pub fn validate_callsign_basic(callsign: &str) -> Result<(), String> {
    // Check for hash callsign (angle brackets) - these are valid
    if callsign.starts_with('<') && callsign.ends_with('>') {
        let inner = &callsign[1..callsign.len()-1];
        if inner.is_empty() {
            return Err(format!("Hash callsign cannot be empty: '{}'", callsign));
        }
        // Inner callsign can contain slashes like PJ4/K1ABC
        if !inner.chars().all(|c| c.is_alphanumeric() || c == '/') {
            return Err(format!("Hash callsign must be alphanumeric with optional slashes: '{}'", callsign));
        }
        return Ok(());
    }
    
    // Strip /R or /P suffix if present for validation
    let base_call = if callsign.ends_with("/R") {
        callsign.strip_suffix("/R").unwrap()
    } else if callsign.ends_with("/P") {
        callsign.strip_suffix("/P").unwrap()
    } else {
        callsign
    };
    
    if base_call.is_empty() || base_call.len() > 11 {
        return Err(format!("Invalid callsign length: '{}'", callsign));
    }
    
    // Must be alphanumeric
    if !base_call.chars().all(|c| c.is_alphanumeric()) {
        return Err(format!("Callsign must be alphanumeric: '{}'", callsign));
    }
    
    Ok(())
}

/// Check if a callsign requires Type 4 (NonStandardCall) encoding
/// Returns true for:
/// - Callsigns with slashes (except /P and /R suffixes)
/// - Callsigns longer than 6 characters (standard limit is 3-6)
pub fn is_nonstandard_callsign(callsign: &str) -> bool {
    // Hash callsigns are not considered non-standard themselves
    if callsign.starts_with('<') && callsign.ends_with('>') {
        return false;
    }
    
    // Callsigns with slashes (except /P and /R) are non-standard
    if callsign.contains('/') && !callsign.ends_with("/P") && !callsign.ends_with("/R") {
        return true;
    }
    
    // Strip /R or /P suffix if present
    let base_call = if callsign.ends_with("/R") {
        callsign.strip_suffix("/R").unwrap()
    } else if callsign.ends_with("/P") {
        callsign.strip_suffix("/P").unwrap()
    } else {
        callsign
    };
    
    // Callsigns longer than 6 characters need Type 4 encoding
    base_call.len() > 6
}

/// Basic validation for grid squares and signal reports
/// Standard grid format: 2 letters + 2 digits (e.g., DM42, FN31)
/// Signal report format: +NN or -NN (e.g., +10, -15)
/// Special codes: RRR, RR73, 73
pub fn validate_grid_basic(grid_or_report: &str) -> Result<(), String> {
    let trimmed = grid_or_report.trim();
    
    // Check for special codes
    if trimmed == "RRR" || trimmed == "RR73" || trimmed == "73" || trimmed.is_empty() {
        return Ok(());
    }
    
    // Check for signal report
    if trimmed.starts_with('+') || trimmed.starts_with('-') {
        // Parse to validate it's a valid number
        let _: i16 = trimmed.parse()
            .map_err(|_| format!("Invalid signal report: '{}'", grid_or_report))?;
        return Ok(());
    }
    
    // Must be a grid square
    if trimmed.len() != 4 {
        return Err(format!("Grid square must be 4 characters: '{}'", grid_or_report));
    }
    
    let chars: Vec<char> = trimmed.chars().collect();
    
    // First two must be letters
    if !chars[0].is_ascii_alphabetic() || !chars[1].is_ascii_alphabetic() {
        return Err(format!("Grid square must start with two letters: '{}'", grid_or_report));
    }
    
    // Last two must be digits
    if !chars[2].is_ascii_digit() || !chars[3].is_ascii_digit() {
        return Err(format!("Grid square must end with two digits: '{}'", grid_or_report));
    }
    
    Ok(())
}
