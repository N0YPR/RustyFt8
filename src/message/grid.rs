/// Grid square (Maidenhead locator) and signal report encoding/decoding functions
/// 
/// Implements WSJT-X grid square and signal report encoding into 15-bit integers.

use alloc::vec::Vec;
use alloc::format;
use alloc::string::{String, ToString};

/// Maximum grid square value (18*18*10*10 = 32400)
/// Values above this are signal reports and special codes
const MAXGRID4: u16 = 32400;

/// Special code offsets (added to MAXGRID4)
const BLANK_CODE: u16 = 1;   // Empty string (2-word message)
const RRR_CODE: u16 = 2;     // "RRR" acknowledgment
const RR73_CODE: u16 = 3;    // "RR73" acknowledgment (special, handled as grid)
const CODE_73: u16 = 4;      // "73" sign-off

/// Encode a grid square or signal report to 15 bits
/// 
/// Handles:
/// - Grid squares: "DM42", "FN31", etc. (encoded as 0-32399)
/// - Signal reports: "+10", "-15", "R+10", "R-15" (encoded as MAXGRID4 + offset)
/// - Special codes: "RRR", "RR73", "73" (encoded as MAXGRID4 + 1, 2, or 3)
///
/// From WSJT-X packjt77.f90:
/// ```fortran
/// ! For grid squares:
/// j1=(ichar(grid4(1:1))-ichar('A'))*18*10*10
/// j2=(ichar(grid4(2:2))-ichar('A'))*10*10
/// j3=(ichar(grid4(3:3))-ichar('0'))*10
/// j4=(ichar(grid4(4:4))-ichar('0'))
/// igrid4=j1+j2+j3+j4
/// 
/// ! For signal reports:
/// if(irpt.ge.-50 .and. irpt.le.-31) irpt=irpt+101
/// irpt=irpt+35
/// igrid4=MAXGRID4 + irpt
/// ```
pub fn encode_grid(grid_or_report: &str) -> Result<u16, String> {
    let trimmed = grid_or_report.trim();
    
    // Handle BLANK (empty string means 2-word message like "CALL1 CALL2")
    if trimmed.is_empty() {
        return Ok(MAXGRID4 + BLANK_CODE);
    }
    
    // Check for signal report (starts with + or -)
    if trimmed.starts_with('+') || trimmed.starts_with('-') {
        return encode_signal_report(trimmed);
    }
    
    // Try to encode as a grid square first (4 characters)
    if trimmed.len() == 4 {
        // Check if it could be a valid grid square
        let chars: Vec<char> = trimmed.chars().collect();
        if chars[0].is_ascii_alphabetic() && chars[1].is_ascii_alphabetic() &&
           chars[2].is_ascii_digit() && chars[3].is_ascii_digit() {
            // It's a valid grid square format (including "RR73", "AA00", etc.)
            return encode_grid_square(trimmed);
        }
    }
    
    // Check for special codes (only if not a valid grid square)
    match trimmed {
        "RRR" => return Ok(MAXGRID4 + RRR_CODE),
        "73" => return Ok(MAXGRID4 + CODE_73),
        _ => {}
    }
    
    // If we get here, it's an unrecognized format
    Err(format!("Invalid grid/report format: '{}' (expected: grid AA00-RR99, signal report +/-NN, or special code RRR/73)", trimmed))
}

/// Encode a grid square (internal helper)
fn encode_grid_square(grid: &str) -> Result<u16, String> {
    // Convert to uppercase for case-insensitive matching
    let grid_upper = grid.to_uppercase();
    let chars: Vec<char> = grid_upper.chars().collect();

    if chars.len() != 4 {
        return Err(format!("Grid must be 4 characters (format: AA00-RR99): '{}'", grid));
    }

    // Convert letters A-R to 0-17
    let c1 = (chars[0] as u32 - 'A' as u32) as u16;
    let c2 = (chars[1] as u32 - 'A' as u32) as u16;
    
    // Convert digits to 0-9
    let c3 = chars[2].to_digit(10).ok_or_else(|| format!("Invalid digit in grid: '{}'", chars[2]))? as u16;
    let c4 = chars[3].to_digit(10).ok_or_else(|| format!("Invalid digit in grid: '{}'", chars[3]))? as u16;
    
    // Validate ranges
    if c1 > 17 || c2 > 17 {
        return Err(format!("Grid letters must be A-R: '{}' (got '{}{}', valid range: AA-RR)", grid, chars[0], chars[1]));
    }
    
    // Encode using WSJT-X formula
    let j1 = c1 * 18 * 10 * 10;
    let j2 = c2 * 10 * 10;
    let j3 = c3 * 10;
    let j4 = c4;
    let igrid4 = j1 + j2 + j3 + j4;
    
    Ok(igrid4)
}

/// Encode a signal report to 15 bits (internal helper)
fn encode_signal_report(report: &str) -> Result<u16, String> {
    // Parse the signal report value
    let report_str = if report.starts_with('R') {
        &report[1..]  // Strip R prefix
    } else {
        report
    };
    
    let irpt: i16 = report_str.parse()
        .map_err(|_| format!("Invalid signal report: '{}'", report))?;
    
    // Validate range: -50 to +49 normally, but -31 to -50 get special handling
    if irpt < -50 || irpt > 49 {
        return Err(format!("Signal report out of range (-50 to +49): {}", irpt));
    }
    
    // Apply WSJT-X encoding formula
    let mut encoded_irpt = irpt;
    if encoded_irpt >= -50 && encoded_irpt <= -31 {
        encoded_irpt += 101;
    }
    encoded_irpt += 35;
    
    let igrid4 = MAXGRID4 + (encoded_irpt as u16);
    Ok(igrid4)
}

/// Decode a 15-bit grid value back to a grid square or signal report string
/// 
/// Reverses the encoding process to reconstruct the original value.
/// 
/// From WSJT-X unpackjt77.f90:
/// ```fortran
/// if(igrid4.le.MAXGRID4) then
///   call to_grid4(igrid4,grid4,ok)
/// else
///   irpt = igrid4 - MAXGRID4
///   ! Decode signal report from irpt
/// endif
/// ```
pub fn decode_grid(igrid4: u16) -> Result<String, String> {
    // Check if it's a grid square or signal report/special code
    if igrid4 <= MAXGRID4 {
        // It's a grid square (note: MAXGRID4 itself would be an invalid grid)
        return decode_grid_square(igrid4);
    }
    
    // It's a signal report or special code
    let irpt = igrid4 - MAXGRID4;

    // Check for special codes
    match irpt {
        BLANK_CODE => Ok(String::new()),
        RRR_CODE => Ok("RRR".to_string()),
        RR73_CODE => Ok("RR73".to_string()),
        CODE_73 => Ok("73".to_string()),
        _ => decode_signal_report(irpt),
    }
}

/// Decode a grid square value (internal helper)
fn decode_grid_square(igrid4: u16) -> Result<String, String> {
    // Reverse the encoding formula
    // igrid4 = c1*1800 + c2*100 + c3*10 + c4
    
    let mut value = igrid4;
    
    // Extract j4 (last digit, 0-9)
    let c4 = (value % 10) as u8;
    value /= 10;
    
    // Extract j3 (third char, digit 0-9)
    let c3 = (value % 10) as u8;
    value /= 10;
    
    // Extract j2 (second letter, A-R)
    let c2 = (value % 18) as u8;
    value /= 18;
    
    // Extract j1 (first letter, A-R)
    let c1 = value as u8;
    
    // Validate ranges
    if c1 > 17 || c2 > 17 || c3 > 9 || c4 > 9 {
        return Err(format!("Invalid grid value: {}", igrid4));
    }
    
    // Convert back to characters
    let ch1 = (b'A' + c1) as char;
    let ch2 = (b'A' + c2) as char;
    let ch3 = (b'0' + c3) as char;
    let ch4 = (b'0' + c4) as char;
    
    Ok(format!("{}{}{}{}", ch1, ch2, ch3, ch4))
}

/// Decode a signal report value (internal helper)
fn decode_signal_report(irpt: u16) -> Result<String, String> {
    // Reverse the encoding: irpt was (report + 35), with special handling for -50 to -31
    let mut report = (irpt as i16) - 35;
    
    // Reverse the special handling for -50 to -31 range
    if report >= 51 && report <= 70 {
        report -= 101;
    }
    
    // Validate range
    if report < -50 || report > 49 {
        return Err(format!("Invalid signal report value: {}", irpt));
    }
    
    // Format with sign and zero-padding to match WSJT-X output
    // WSJT-X always outputs reports with leading zero for single digits: +09, -09, etc.
    if report >= 0 {
        Ok(format!("+{:02}", report))
    } else {
        Ok(format!("-{:02}", -report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_grid() {
        // DM42: D=3, M=12, 4=4, 2=2
        // j1 = 3*1800 = 5400
        // j2 = 12*100 = 1200  
        // j3 = 4*10 = 40
        // j4 = 2
        // Total = 6642
        assert_eq!(encode_grid("DM42").unwrap(), 6642);
        
        // FN31: F=5, N=13, 3=3, 1=1
        // j1 = 5*1800 = 9000
        // j2 = 13*100 = 1300
        // j3 = 3*10 = 30
        // j4 = 1
        // Total = 10331
        assert_eq!(encode_grid("FN31").unwrap(), 10331);
    }

    #[test]
    fn test_invalid_grids() {
        assert!(encode_grid("DM4").is_err());   // Too short
        assert!(encode_grid("DMAB").is_err());  // Invalid digits
    }
    
    #[test]
    fn test_decode_grid() {
        // Test round-trip encoding/decoding
        assert_eq!(decode_grid(6642).unwrap(), "DM42");
        assert_eq!(decode_grid(10331).unwrap(), "FN31");
        
        // Test AA00 (encodes to 0)
        let val = encode_grid("AA00").unwrap();
        assert_eq!(val, 0);
        assert_eq!(decode_grid(val).unwrap(), "AA00");
        
        // Test RR99 (max values)
        let val = encode_grid("RR99").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "RR99");
    }
    
    #[test]
    fn test_grid_roundtrip() {
        let grids = vec!["DM42", "FN31", "AA00", "RR99", "JN76", "EM00"];
        for grid in grids {
            let encoded = encode_grid(grid).unwrap();
            let decoded = decode_grid(encoded).unwrap();
            assert_eq!(decoded, grid, "Failed roundtrip for {}", grid);
        }
    }

    #[test]
    fn test_case_insensitive_grids() {
        // Test that lowercase grids work the same as uppercase
        let lowercase_grids = vec!["dm42", "fn31", "aa00", "rr99"];
        let uppercase_grids = vec!["DM42", "FN31", "AA00", "RR99"];

        for (lower, upper) in lowercase_grids.iter().zip(uppercase_grids.iter()) {
            let encoded_lower = encode_grid(lower).unwrap();
            let encoded_upper = encode_grid(upper).unwrap();
            assert_eq!(encoded_lower, encoded_upper, "Case mismatch for {}/{}", lower, upper);

            // Decode should always return uppercase
            let decoded = decode_grid(encoded_lower).unwrap();
            assert_eq!(decoded, *upper, "Decoded value should be uppercase");
        }
    }

    #[test]
    fn test_signal_reports() {
        // Test positive signal reports
        let val = encode_grid("+10").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "+10");

        let val = encode_grid("+00").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "+00");

        let val = encode_grid("+49").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "+49");

        // Test negative signal reports
        let val = encode_grid("-10").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "-10");

        let val = encode_grid("-01").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "-01");

        let val = encode_grid("-30").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "-30");
    }

    #[test]
    fn test_signal_report_boundaries() {
        // Test boundary values
        let val = encode_grid("-50").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "-50");

        let val = encode_grid("+49").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "+49");

        // Test the special -31 to -50 range that gets special encoding
        let val = encode_grid("-31").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "-31");

        let val = encode_grid("-40").unwrap();
        assert_eq!(decode_grid(val).unwrap(), "-40");

        // Test out of range
        assert!(encode_grid("-51").is_err());
        assert!(encode_grid("+50").is_err());
    }

    #[test]
    fn test_signal_report_roundtrip() {
        // Test roundtrip for various signal reports
        let reports = vec!["-50", "-30", "-15", "-08", "+00", "+10", "+20", "+49"];
        for report in reports {
            let encoded = encode_grid(report).unwrap();
            let decoded = decode_grid(encoded).unwrap();
            assert_eq!(decoded, report, "Failed roundtrip for {}", report);
        }
    }

    #[test]
    fn test_special_codes() {
        // Test BLANK (empty string)
        let val = encode_grid("").unwrap();
        assert_eq!(val, MAXGRID4 + BLANK_CODE);
        assert_eq!(decode_grid(val).unwrap(), "");

        // Test RRR
        let val = encode_grid("RRR").unwrap();
        assert_eq!(val, MAXGRID4 + RRR_CODE);
        assert_eq!(decode_grid(val).unwrap(), "RRR");

        // Test 73
        let val = encode_grid("73").unwrap();
        assert_eq!(val, MAXGRID4 + CODE_73);
        assert_eq!(decode_grid(val).unwrap(), "73");

        // Test RR73 (this is a special case - it's a valid grid square!)
        // RR73: R=17, R=17, 7=7, 3=3
        // j1 = 17*1800 = 30600
        // j2 = 17*100 = 1700
        // j3 = 7*10 = 70
        // j4 = 3
        // Total = 32373
        let val = encode_grid("RR73").unwrap();
        assert_eq!(val, 32373);  // It's encoded as a grid square, not a special code
        assert_eq!(decode_grid(val).unwrap(), "RR73");
    }

    #[test]
    fn test_special_code_roundtrip() {
        let codes = vec!["", "RRR", "73"];
        for code in codes {
            let encoded = encode_grid(code).unwrap();
            let decoded = decode_grid(encoded).unwrap();
            assert_eq!(decoded, code, "Failed roundtrip for special code '{}'", code);
        }
    }

    #[test]
    fn test_edge_cases() {
        // Test that RR73 is treated as a grid square, not the special code
        let val_rr73 = encode_grid("RR73").unwrap();
        assert!(val_rr73 < MAXGRID4, "RR73 should be encoded as a grid square");

        // Verify direct decode of special code values
        assert_eq!(decode_grid(MAXGRID4 + RR73_CODE).unwrap(), "RR73");

        // Test max valid grid RR99
        let val = encode_grid("RR99").unwrap();
        assert_eq!(val, 32399);  // Max grid value
        assert!(val < MAXGRID4);

        // Test invalid inputs
        assert!(encode_grid("XY12").is_err());  // X, Y > R
        assert!(encode_grid("DM").is_err());    // Too short
        assert!(encode_grid("DM423").is_err()); // Too long
        assert!(encode_grid("DMAB").is_err());  // Invalid digits
    }
}
