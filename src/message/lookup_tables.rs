/// ARRL Field Day section codes (1-indexed, 86 total)
/// From WSJT-X packjt77.f90
pub const ARRL_SECTIONS: [&str; 86] = [
    "AB", "AK", "AL", "AR", "AZ", "BC", "CO", "CT", "DE", "EB",      // 1-10
    "EMA", "ENY", "EPA", "EWA", "GA", "GH", "IA", "ID", "IL", "IN",  // 11-20
    "KS", "KY", "LA", "LAX", "NS", "MB", "MDC", "ME", "MI", "MN",    // 21-30
    "MO", "MS", "MT", "NC", "ND", "NE", "NFL", "NH", "NL", "NLI",    // 31-40
    "NM", "NNJ", "NNY", "TER", "NTX", "NV", "OH", "OK", "ONE", "ONN", // 41-50
    "ONS", "OR", "ORG", "PAC", "PR", "QC", "RI", "SB", "SC", "SCV",  // 51-60
    "SD", "SDG", "SF", "SFL", "SJV", "SK", "SNJ", "STX", "SV", "TN",  // 61-70
    "UT", "VA", "VI", "VT", "WCF", "WI", "WMA", "WNY", "WPA", "WTX", // 71-80
    "WV", "WWA", "WY", "DX", "PE", "NB",                              // 81-86
];

/// ARRL RTTY Roundup state/province codes (1-indexed, 171 total)
/// From WSJT-X packjt77.f90 cmult array
pub const RTTY_STATES: [&str; 171] = [
    "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "FL", "GA",      // 1-10
    "HI", "ID", "IL", "IN", "IA", "KS", "KY", "LA", "ME", "MD",      // 11-20
    "MA", "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ",      // 21-30
    "NM", "NY", "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC",      // 31-40
    "SD", "TN", "TX", "UT", "VT", "VA", "WA", "WV", "WI", "WY",      // 41-50
    "NB", "NS", "QC", "ON", "MB", "SK", "AB", "BC", "NWT", "NF",     // 51-60
    "LB", "NU", "YT", "PEI", "DC", "DR", "FR", "GD", "GR", "OV",     // 61-70
    "ZH", "ZL",                                                        // 71-72
    "X01", "X02", "X03", "X04", "X05", "X06", "X07", "X08", "X09", "X10", // 73-82
    "X11", "X12", "X13", "X14", "X15", "X16", "X17", "X18", "X19", "X20", // 83-92
    "X21", "X22", "X23", "X24", "X25", "X26", "X27", "X28", "X29", "X30", // 93-102
    "X31", "X32", "X33", "X34", "X35", "X36", "X37", "X38", "X39", "X40", // 103-112
    "X41", "X42", "X43", "X44", "X45", "X46", "X47", "X48", "X49", "X50", // 113-122
    "X51", "X52", "X53", "X54", "X55", "X56", "X57", "X58", "X59", "X60", // 123-132
    "X61", "X62", "X63", "X64", "X65", "X66", "X67", "X68", "X69", "X70", // 133-142
    "X71", "X72", "X73", "X74", "X75", "X76", "X77", "X78", "X79", "X80", // 143-152
    "X81", "X82", "X83", "X84", "X85", "X86", "X87", "X88", "X89", "X90", // 153-162
    "X91", "X92", "X93", "X94", "X95", "X96", "X97", "X98", "X99",       // 163-171
];

/// Find ARRL section code index (1-86) from abbreviation
pub fn arrl_section_to_index(section: &str) -> Option<u8> {
    ARRL_SECTIONS.iter()
        .position(|&s| s.eq_ignore_ascii_case(section))
        .map(|idx| (idx + 1) as u8)
}

/// Get ARRL section abbreviation from index (1-86)
pub fn arrl_section_from_index(index: u8) -> Option<&'static str> {
    if index >= 1 && index <= 86 {
        Some(ARRL_SECTIONS[(index - 1) as usize])
    } else {
        None
    }
}

/// Get RTTY Roundup state/province index from abbreviation
pub fn rtty_state_to_index(state: &str) -> Option<u16> {
    RTTY_STATES.iter()
        .position(|&s| s.eq_ignore_ascii_case(state))
        .map(|idx| 8000 + (idx + 1) as u16)
}

/// Get RTTY Roundup state/province abbreviation from index (8001-8171)
pub fn rtty_state_from_index(index: u16) -> Option<&'static str> {
    if index >= 8001 && index <= 8171 {
        Some(RTTY_STATES[(index - 8001) as usize])
    } else {
        None
    }
}
