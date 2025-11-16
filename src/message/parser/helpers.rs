//! Helper Functions for FT8 Message Parsing
//!
//! This module provides common helper functions for parsing FT8 messages, including
//! callsign suffix handling (/R and /P) and CQ message parsing. These utilities are
//! used by the main parser orchestration to process different message formats.

use crate::message::types::MessageVariant;
use super::validators::{validate_callsign_basic, validate_grid_basic};

/// Parse callsign suffix and return (base_callsign, has_suffix, is_p_suffix)
///
/// This function handles both /R and /P suffixes:
/// - /R suffix: has_suffix=true, is_p_suffix=false (uses Type 1/Standard encoding)
/// - /P suffix: has_suffix=true, is_p_suffix=true (uses Type 2 encoding)
/// - No suffix: has_suffix=false, is_p_suffix=false
pub(super) fn parse_suffix(callsign: &str) -> (String, bool, bool) {
    if callsign.ends_with("/R") {
        (callsign.strip_suffix("/R").unwrap().to_string(), true, false)
    } else if callsign.ends_with("/P") {
        (callsign.strip_suffix("/P").unwrap().to_string(), true, true)
    } else {
        (callsign.to_string(), false, false)
    }
}

/// Strip suffix from callsign (simpler version that doesn't distinguish /R from /P)
/// Returns (base_callsign, has_suffix)
pub(super) fn strip_suffix(callsign: &str) -> (String, bool) {
    if callsign.ends_with("/R") || callsign.ends_with("/P") {
        (callsign[..callsign.len()-2].to_string(), true)
    } else {
        (callsign.to_string(), false)
    }
}

/// Parse CQ message and return appropriate MessageVariant
///
/// Handles both 3-word and 4-word CQ messages:
/// - 3-word: "CQ CALLSIGN GRID"
/// - 4-word: "CQ MODIFIER CALLSIGN GRID" (e.g., "CQ SOTA N0YPR DM42")
///
/// Automatically selects Type 1 (Standard) or Type 2 (EuVhfContest) based on suffix:
/// - /R suffix uses Type 1
/// - /P suffix uses Type 2
pub(super) fn parse_cq_message(cq_prefix: &str, callsign_str: &str, grid_str: &str) -> Result<MessageVariant, String> {
    let callsign = callsign_str.to_uppercase();
    let grid = grid_str.to_uppercase();

    // Parse suffix to determine message type
    let (base_callsign, has_suffix, is_p_suffix) = parse_suffix(&callsign);

    // Validate callsign and grid
    validate_callsign_basic(&base_callsign)?;
    validate_grid_basic(&grid)?;

    // Use Type 2 for /P suffixes, Type 1 (Standard) for /R or no suffix
    if is_p_suffix {
        Ok(MessageVariant::EuVhfContestType2 {
            call1: cq_prefix.to_string(),
            call1_suffix: false,
            call2: base_callsign,
            call2_suffix: true,
            r_flag: false,
            grid_or_report: grid,
        })
    } else {
        Ok(MessageVariant::Standard {
            call1: cq_prefix.to_string(),
            call1_suffix: false,
            call2: base_callsign,
            call2_suffix: has_suffix,
            r_flag: false,
            grid_or_report: grid,
        })
    }
}
