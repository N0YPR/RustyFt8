use std::fmt::Display;
use snafu::prelude::*;
use super::radix::{FromMixedRadixStr, ToStrMixedRadix};

const CHAR_TABLE_ALPHA_UPPER:&str = "ABCDEFGHIJKLMNOPQR";
const CHAR_TABLE_ALPHA_LOWER:&str = "abcdefghijklmnopqrstuvwx";
const CHAR_TABLE_NUMERIC:&str = "0123456789";
const MAX_GRID_4:u32 = 32400;
const OTHER_REPORTS: [&str; 4] = ["", "RRR", "RR73", "73"];

pub struct Report {
    pub report: String,
    pub is_ack: bool,
    pub is_g15: bool,
    pub is_g25: bool,
    pub is_other: bool,
    pub packed_bits: u32,
    pub other_bits: u32,
}

impl Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.report)
    }
}

impl Report {
    pub fn try_from_packed_15(value:u32) -> Result<Self, InvalidValueError> {
        let grid:String;
        let is_ack = false;
        let is_g15 = true;
        let is_g25 = false;
        let packed_bits:u32 = value;
        let is_other:bool;
        let other_bits:u32;

        // 4-char reports
        if value <= MAX_GRID_4 {
            let radix_tables = [
                CHAR_TABLE_ALPHA_UPPER,
                CHAR_TABLE_ALPHA_UPPER,
                CHAR_TABLE_NUMERIC,
                CHAR_TABLE_NUMERIC
            ];
            grid = match value.to_str_mixed_radix(&radix_tables) {
                Ok(v) => v,
                Err(_) => {
                    return Err(InvalidValueError { value: value });
                }
            };
            is_other = false;
            other_bits = 0;
        }

        // special reports
        else if value > MAX_GRID_4 && value < MAX_GRID_4 + OTHER_REPORTS.len() as u32 + 1 {
            let other_index = value - MAX_GRID_4 - 1u32;
            match OTHER_REPORTS.iter().nth(other_index as usize) {
                Some(v) => {
                    grid = (*v).to_owned();
                    other_bits = other_index;
                },
                None => {
                    return Err(InvalidValueError { value: value });
                }
            };
            is_other = true;
        } 
        
        // signal reports
        else if value >= MAX_GRID_4 + 35u32 && value <= MAX_GRID_4 + 65u32 {
            //let packed_bits = (MAX_GRID_4 as i32 + 35i32 + report) as u32;
            let report = value as i32 - MAX_GRID_4 as i32 - 35i32;
            grid = format!("{:>+0width$.prec$}", report, width=3, prec=0);
            is_other = false;
            other_bits = 0;
        } 
        
        // error
        else {
            return Err(InvalidValueError { value });
        }

        let g = Report {
            report: grid,
            is_ack,
            is_g15,
            is_g25,
            is_other,
            packed_bits,
            other_bits
        };

        return Ok(g);
    }

    pub fn try_from_packed_25(value:u32) -> Result<Self, InvalidValueError> {
        let grid:String;
        let is_ack = false;
        let is_g15 = false;
        let is_g25 = true;
        let is_other = false;
        let packed_bits:u32 = value;
        let other_bits = 0;

        let radix_tables = [
            CHAR_TABLE_ALPHA_UPPER,
            CHAR_TABLE_ALPHA_UPPER,
            CHAR_TABLE_NUMERIC,
            CHAR_TABLE_NUMERIC,
            CHAR_TABLE_ALPHA_LOWER,
            CHAR_TABLE_ALPHA_LOWER
        ];
        grid = match value.to_str_mixed_radix(&radix_tables) {
            Ok(v) => v,
            Err(_) => {
                return Err(InvalidValueError { value });
            }
        };

        let g = Report {
            report: grid,
            is_ack,
            is_g15,
            is_g25,
            is_other,
            packed_bits,
            other_bits
        };

        return Ok(g);
    }

    pub fn try_from_report_str(report_str:&str) -> Result<Self, InvalidStringError> {

        if let Ok(report) = try_from_location(report_str) {
            return Ok(report);
        }

        if let Ok(report) = try_from_special(report_str) {
            return Ok(report);
        }

        if let Ok(report) = try_from_signal_report(report_str) {
            return Ok(report);
        }

        return Err(InvalidStringError { value: report_str.to_owned() });
    }

    
}

fn try_from_signal_report(report_str:&str) -> Result<Report, InvalidStringError> {
    let is_ack = report_str.starts_with("R");

    let string_to_parse = if is_ack {&report_str[1..]} else { report_str };

    if (!string_to_parse.starts_with("-") && !string_to_parse.starts_with("+")) && string_to_parse.len() != 3 {
        return Err(InvalidStringError { value: report_str.to_owned() });
    }

    let report_int =  match string_to_parse.parse::<i32>() {
        Ok(v) => v,
        Err(_) => {
            return Err(InvalidStringError { value: report_str.to_owned() });
        }
    };

    if report_int < -30 || report_int > 30 {
        return Err(InvalidStringError { value: report_str.to_owned() });
    }

    //let r = format!("{:>+0width$.prec$}", report_int, width=3, prec=0);
    let is_g15 = true;
    let is_g25 = false;
    let is_other = false;
    let packed_bits = (MAX_GRID_4 as i32 + 35i32 + report_int) as u32;
    let other_bits = 0;

    return Ok(Report {
        report: report_str.to_owned(),
        is_ack,
        is_g15,
        is_g25,
        is_other,
        packed_bits,
        other_bits
    });
}

fn try_from_location(location:&str) -> Result<Report, InvalidStringError> {
    let is_other = location == "RR73";
    let other_bits = if is_other {2} else {0};

    let is_ack:bool;
    let location_to_parse:&str;
    if location.starts_with("R ") {
        is_ack = true;
        location_to_parse = &location[2..];
    } else {
        is_ack = false;
        location_to_parse = location;
    }

    let is_g15 = location_to_parse.len() == 4;
    let is_g25 = location_to_parse.len() == 6;

    if !is_g15 && !is_g25 {
        return Err(InvalidStringError { value: location_to_parse.to_owned() });
    }

    let packed_bits:u32;
    if is_g15 {
        let radix_tables = [
            CHAR_TABLE_ALPHA_UPPER,
            CHAR_TABLE_ALPHA_UPPER,
            CHAR_TABLE_NUMERIC,
            CHAR_TABLE_NUMERIC
        ];
        packed_bits = match u32::from_mixed_radix_str(location_to_parse, &radix_tables) {
            Ok(value) => value,
            Err(e) => {
                return Err(InvalidStringError { value: location_to_parse.to_owned() });
            }
        };
    } else if is_g25 {
        let radix_tables = [
            CHAR_TABLE_ALPHA_UPPER,
            CHAR_TABLE_ALPHA_UPPER,
            CHAR_TABLE_NUMERIC,
            CHAR_TABLE_NUMERIC,
            CHAR_TABLE_ALPHA_LOWER,
            CHAR_TABLE_ALPHA_LOWER
        ];
        packed_bits = match u32::from_mixed_radix_str(location_to_parse, &radix_tables) {
            Ok(value) => value,
            Err(e) => {
                return Err(InvalidStringError { value: location_to_parse.to_owned() });
            }
        };
    } else {
        packed_bits = 0; // will never happen
    }

    let r = Report {
        report: location.to_owned(),
        is_ack,
        is_g15,
        is_g25,
        is_other,
        packed_bits,
        other_bits
    };
    return Ok(r);
}

fn try_from_special(special:&str) -> Result<Report, InvalidStringError> {
    if !OTHER_REPORTS.contains(&special) {
        return Err(InvalidStringError { value: special.to_owned() });
    }
    
    let other_index = OTHER_REPORTS.iter().position(|&r| r == special).unwrap();

    let packed_bits = MAX_GRID_4 + other_index as u32 + 1;
    let is_ack = false;
    let is_g15 = true;
    let is_g25 = false;
    let is_other = true;
    let other_bits = other_index as u32;

    let r = Report {
        report: special.to_owned(),
        is_ack,
        is_g15,
        is_g25,
        is_other,
        packed_bits,
        other_bits
    };
    return Ok(r);
}

#[derive(Debug, Snafu)]
#[snafu(display("Value {value} is not a valid Report"))]
pub struct InvalidValueError {
    value: u32,
}

#[derive(Debug, Snafu)]
#[snafu(display("String {value} is not a valid Report"))]
pub struct InvalidStringError {
    value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    mod try_from_packed_15 {
        use super::*;

        mod with_32402 {
            use super::*;

            #[test]
            fn grid_is_rrr() {
                let g = Report::try_from_packed_15(32402).unwrap();
                assert_eq!(g.report, "RRR");
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_packed_15(32402).unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_packed_15(32402).unwrap();
                assert!(!g.is_g25);
            }

            #[test]
            fn packed_bits_is_32402() {
                let g = Report::try_from_packed_15(32402).unwrap();
                assert_eq!(g.packed_bits, 32402);
            }
        }

        mod with_32435 {
            use super::*;

            #[test]
            fn grid_is_plus00() {
                let g = Report::try_from_packed_15(32435).unwrap();
                assert_eq!(g.report, "+00");
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_packed_15(32435).unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_packed_15(32435).unwrap();
                assert!(!g.is_g25);
            }

            #[test]
            fn packed_bits_is_32402() {
                let g = Report::try_from_packed_15(32435).unwrap();
                assert_eq!(g.packed_bits, 32435);
            }
        }

        mod with_4987 {
            use super::*;

            #[test]
            fn grid_is_cn87() {
                let g = Report::try_from_packed_15(4987).unwrap();
                assert_eq!(g.report, "CN87");
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_packed_15(4987).unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_packed_15(4987).unwrap();
                assert!(!g.is_g25);
            }

            #[test]
            fn packed_bits_is_4987() {
                let g = Report::try_from_packed_15(4987).unwrap();
                assert_eq!(g.packed_bits, 4987);
            }
        }

        #[test]
        fn with_2873078_returns_invalid_value() {
            let result = Report::try_from_packed_15(2873078);
            assert!(matches!(result, Err(InvalidValueError { value: 2873078})));
        }
    }

    mod try_from_packed_25 {
        use super::*;

        mod with_2873078 {
            use super::*;

            #[test]
            fn grid_is_cn87xo() {
                let g = Report::try_from_packed_25(2873078).unwrap();
                assert_eq!(g.report, "CN87xo");
            }

            #[test]
            fn is_g15_false() {
                let g = Report::try_from_packed_25(2873078).unwrap();
                assert!(!g.is_g15);
            }

            #[test]
            fn is_g25_true() {
                let g = Report::try_from_packed_25(2873078).unwrap();
                assert!(g.is_g25);
            }

            #[test]
            fn packed_bits_is_2873078() {
                let g = Report::try_from_packed_25(2873078).unwrap();
                assert_eq!(g.packed_bits, 2873078);
            }

        }
    }

    mod try_from_special {
        use super::*;

        mod with_rrr {
            use super::*;

            #[test]
            fn grid_is_rrr() {
                let g = Report::try_from_report_str("RRR").unwrap();
                assert_eq!(g.report, "RRR");
            }

            #[test]
            fn packed_bits_is_32402() {
                let g = Report::try_from_report_str("RRR").unwrap();
                assert_eq!(g.packed_bits, 32402);
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_report_str("RRR").unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_report_str("RRR").unwrap();
                assert!(!g.is_g25);
            }
        }

        mod with_rr73_actually_not_special_but_grid {
            use super::*;

            #[test]
            fn grid_is_rr73() {
                let g = Report::try_from_report_str("RR73").unwrap();
                assert_eq!(g.report, "RR73");
            }

            #[test]
            fn packed_bits_is_32373() {
                let g = Report::try_from_report_str("RR73").unwrap();
                assert_eq!(g.packed_bits, 32373);
                // 0b111111010010011
                // 0b111111001110101
                // 32373
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_report_str("RR73").unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_report_str("RR73").unwrap();
                assert!(!g.is_g25);
            }
        }

        mod with_73 {
            use super::*;

            #[test]
            fn grid_is_73() {
                let g = Report::try_from_report_str("73").unwrap();
                assert_eq!(g.report, "73");
            }

            #[test]
            fn packed_bits_is_32403() {
                let g = Report::try_from_report_str("73").unwrap();
                assert_eq!(g.packed_bits, 32404);
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_report_str("73").unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_report_str("73").unwrap();
                assert!(!g.is_g25);
            }
        }

        mod with_empty {
            use super::*;

            #[test]
            fn grid_is_empty() {
                let g = Report::try_from_report_str("").unwrap();
                assert_eq!(g.report, "");
            }

            #[test]
            fn packed_bits_is_32403() {
                let g = Report::try_from_report_str("").unwrap();
                assert_eq!(g.packed_bits, 32401);
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_report_str("").unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_report_str("").unwrap();
                assert!(!g.is_g25);
            }
        }
    }


    mod try_from_location {
        use super::*;
        
        mod with_cn87 {
            use super::*;

            #[test]
            fn grid_is_cn87() {
                let g = Report::try_from_report_str("CN87").unwrap();
                assert_eq!(g.report, "CN87");
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_report_str("CN87").unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_report_str("CN87").unwrap();
                assert!(!g.is_g25);
            }

            #[test]
            fn packed_bits_is_4987() {
                let g = Report::try_from_report_str("CN87").unwrap();
                assert_eq!(g.packed_bits, 4987);
            }
        }

        #[test]
        fn with_zz99_returns_invalid_char() {
            let g = Report::try_from_report_str("ZZ99");
            assert!(g.is_err());
        }

        #[test]
        fn with_cn8_returns_invalid_length() {
            let g = Report::try_from_report_str("CN8");
            assert!(g.is_err());
        }

        mod with_cn87xo {
            use super::*;

            #[test]
            fn grid_is_cn87xo() {
                let g = Report::try_from_report_str("CN87xo").unwrap();
                assert_eq!(g.report, "CN87xo");
            }

            #[test]
            fn is_g15_false() {
                let g = Report::try_from_report_str("CN87xo").unwrap();
                assert!(!g.is_g15);
            }

            #[test]
            fn is_g25_true() {
                let g = Report::try_from_report_str("CN87xo").unwrap();
                assert!(g.is_g25);
            }

            #[test]
            fn packed_bits_is_4987() {
                let g = Report::try_from_report_str("CN87xo").unwrap();
                assert_eq!(g.packed_bits, 2873078);
            }

        }
    }

    mod try_from_report {
        use super::*;

        mod with_zero {
            use super::*;
            
            #[test]
            fn report_is_plus00() {
                let g = Report::try_from_report_str("+00").unwrap();
                assert_eq!(g.report, "+00");
            }

            #[test]
            fn is_g15_true() {
                let g = Report::try_from_report_str("+00").unwrap();
                assert!(g.is_g15);
            }

            #[test]
            fn is_g25_false() {
                let g = Report::try_from_report_str("+00").unwrap();
                assert!(!g.is_g25);
            }

            #[test]
            fn packed_bits_is_32435() {
                let g = Report::try_from_report_str("+00").unwrap();
                assert_eq!(g.packed_bits, 32435);
            }
        }

        mod with_neg_10 {
            use super::*;

            #[test]
            fn report_is_neg10() {
                let g = Report::try_from_report_str("-10").unwrap();
                assert_eq!(g.report, "-10");
            }

            #[test]
            fn packed_bits_is_32425() {
                let g = Report::try_from_report_str("-10").unwrap();
                assert_eq!(g.packed_bits, 32425);
            }
        }

        mod with_r_neg_20 {
            use super::*;

            #[test]
            fn report_is_r_neg20() {
                let g = Report::try_from_report_str("R-20").unwrap();
                assert_eq!(g.report, "R-20");
            }

            #[test]
            fn packed_bits_is_32415() {
                let g = Report::try_from_report_str("R-20").unwrap();
                //0b111111010011111
                //0000101001001101100111001101_1_1001011111000101011100011111_0_1_111111010011111_001
                assert_eq!(g.packed_bits, 32415);
            }

            #[test]
            fn is_ack_is_true() {
                let g = Report::try_from_report_str("R-20").unwrap();
                assert!(g.is_ack);
            }
        }

        mod with_r_fn42 {
            use super::*;

            #[test]
            fn report_is_r_fn42() {
                let g = Report::try_from_report_str("R FN42").unwrap();
                assert_eq!(g.report, "R FN42");
            }

            #[test]
            fn packed_bits_is_10342() {
                let g = Report::try_from_report_str("R FN42").unwrap();
                //0b010100001100110
                //0000110000101001001110111000000001001101111011110001101011_1_010100001100110_001
                assert_eq!(g.packed_bits, 10342);
            }

            #[test]
            fn is_ack_is_true() {
                let g = Report::try_from_report_str("R FN42").unwrap();
                assert!(g.is_ack);
            }
        }
        
    }
}