use std::fmt::Display;
use snafu::prelude::*;
use crate::constants::{FT8_CHAR_TABLE_GRIDSQUARE_ALPHA, FT8_CHAR_TABLE_GRIDSQUARE_ALPHA_SIX, FT8_CHAR_TABLE_NUMERIC};

use super::radix::{FromMixedRadixStr, ToStrMixedRadix};

const MAX_GRID_4:u32 = 32400;
pub const OTHER_REPORTS: [&str; 4] = ["", "RRR", "RR73", "73"];

#[derive(Debug)]
pub struct Report {
    pub report: String,
    pub is_ack: bool,
    pub is_other: bool,
    pub packed_bits: u32,
}

impl Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_ack {
            //write!(f, "R{}", self.report)
            if self.report.len() == 4 || self.report.len() == 6 {
                write!(f, "R {}", self.report)
            } else {
                write!(f, "R{}", self.report)
            }
        } else {
            write!(f, "{}", self.report)
        }
    }
}

impl Report {
    pub fn try_from_packed_bits(value: u32, num_bits:usize) -> Result<Self, InvalidValueError> {
        if num_bits == 2 {
            // RRR, RR73, 73, or blank
            let report = match OTHER_REPORTS.iter().nth(value as usize) {
                Some(v) => {
                    (*v).to_owned()
                },
                None => {
                    return Err(InvalidValueError { value: value });
                }
            };
            let packed_bits = value;
            return Ok(Report {
                report,
                is_ack: false,
                is_other: true,
                packed_bits
            })
        }

        if num_bits == 15 {
            let report:String;
            let is_ack = false;
            let is_other:bool;
            let packed_bits:u32 = value;
            
            // 4-char reports
            if value <= MAX_GRID_4 {
                println!("4-char reports");
                let radix_tables = [
                    FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
                    FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
                    FT8_CHAR_TABLE_NUMERIC,
                    FT8_CHAR_TABLE_NUMERIC
                ];
                report = match value.to_str_mixed_radix(&radix_tables) {
                    Ok(v) => v,
                    Err(_) => {
                        return Err(InvalidValueError { value: value });
                    }
                };
                is_other = false;
            }

            // special reports
            else if value > MAX_GRID_4 && value < MAX_GRID_4 + OTHER_REPORTS.len() as u32 + 1 {
                let other_index = value - MAX_GRID_4 - 1u32;
                report = match OTHER_REPORTS.iter().nth(other_index as usize) {
                    Some(v) => {
                        (*v).to_owned()
                    },
                    None => {
                        return Err(InvalidValueError { value: value });
                    }
                };
                is_other = true;
            }

            // signal reports low
            else if value >= 32405 && value <= 32485 {
                let r = value as i32 - 32405i32 - 30i32;
                report = format!("{:>+0width$.prec$}", r, width=3, prec=0);
                is_other = false;
            }

            // signal reports high
            else if value >= 32486 && value <= 32505 {
                let r = value as i32 - 32486i32 - 50i32;
                report = format!("{:>+0width$.prec$}", r, width=3, prec=0);
                is_other = false;
            }

            else {
                return Err(InvalidValueError { value });
            }

            return Ok(Report {
                report,
                is_ack,
                is_other,
                packed_bits
            })
        }

        if num_bits == 25 {
            let radix_tables = [
                FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
                FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
                FT8_CHAR_TABLE_NUMERIC,
                FT8_CHAR_TABLE_NUMERIC,
                FT8_CHAR_TABLE_GRIDSQUARE_ALPHA_SIX,
                FT8_CHAR_TABLE_GRIDSQUARE_ALPHA_SIX
            ];
            let report = match value.to_str_mixed_radix(&radix_tables) {
                Ok(v) => v,
                Err(_) => {
                    return Err(InvalidValueError { value });
                }
            };
            return Ok(Report {
                report,
                is_ack: false,
                is_other: false,
                packed_bits: value
            })
        }
        


        Err(InvalidValueError { value })

    }

    pub fn try_from_report_str(report_str:&str, num_bits:usize) -> Result<Self, InvalidStringError> {

        if num_bits == 2 {
            if let Ok(report) = try_from_special_2(report_str) {
                return Ok(report);
            }
            return Err(InvalidStringError { value: report_str.to_owned() });
        }

        if num_bits == 15 {
            if let Ok(report) = try_from_location(report_str, 15) {
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

        if num_bits == 25 {
            if let Ok(report) = try_from_location(report_str, 25) {
                return Ok(report);
            }

            return Err(InvalidStringError { value: report_str.to_owned() });
            
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

    if report_int < -50 || report_int > 50 {
        return Err(InvalidStringError { value: report_str.to_owned() });
    }

    let is_other = false;

    let packed_bits:u32;
    if report_int >= -30 && report_int <= 50 {
        packed_bits = (MAX_GRID_4 as i32 + 35i32 + report_int) as u32
    } else if report_int >= -50 && report_int <= -31 {
        packed_bits = (32486i32 + 50i32 + report_int) as u32
    } else {
        return Err(InvalidStringError { value: report_str.to_owned() });
    }

    return Ok(Report {
        report: format!("{:+03}", report_int),
        is_ack,
        is_other,
        packed_bits,
    });
}

fn try_from_location(location:&str, num_bits:usize) -> Result<Report, InvalidStringError> {
    let is_other = location == "RR73";
    let is_ack:bool;
    let location_to_parse:&str;
    if location.starts_with("R ") {
        is_ack = true;
        location_to_parse = &location[2..];
    } else {
        is_ack = false;
        location_to_parse = location;
    }

    let is_g15 = location_to_parse.len() == 4 && num_bits == 15;
    let is_g25 = location_to_parse.len() == 6 && num_bits == 25;

    if !is_g15 && !is_g25 {
        return Err(InvalidStringError { value: location_to_parse.to_owned() });
    }

    let packed_bits:u32;
    if is_g15 {
        let radix_tables = [
            FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
            FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
            FT8_CHAR_TABLE_NUMERIC,
            FT8_CHAR_TABLE_NUMERIC
        ];
        packed_bits = match u32::from_mixed_radix_str(location_to_parse, &radix_tables) {
            Ok(value) => value,
            Err(e) => {
                return Err(InvalidStringError { value: location_to_parse.to_owned() });
            }
        };
    } else if is_g25 {
        let radix_tables = [
            FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
            FT8_CHAR_TABLE_GRIDSQUARE_ALPHA,
            FT8_CHAR_TABLE_NUMERIC,
            FT8_CHAR_TABLE_NUMERIC,
            FT8_CHAR_TABLE_GRIDSQUARE_ALPHA_SIX,
            FT8_CHAR_TABLE_GRIDSQUARE_ALPHA_SIX
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
        report: location_to_parse.to_owned(),
        is_ack,
        is_other,
        packed_bits,
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
    let is_other = true;

    let r = Report {
        report: special.to_owned(),
        is_ack,
        is_other,
        packed_bits,
    };
    return Ok(r);
}

fn try_from_special_2(special:&str) -> Result<Report, InvalidStringError> {
    if !OTHER_REPORTS.contains(&special) {
        return Err(InvalidStringError { value: special.to_owned() });
    }
    
    let other_index = OTHER_REPORTS.iter().position(|&r| r == special).unwrap();

    let packed_bits = other_index as u32;
    let is_ack = false;
    let is_other = true;

    let r = Report {
        report: special.to_owned(),
        is_ack,
        is_other,
        packed_bits,
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

    macro_rules! generate_tests { ($name:ident, $report:expr, $packed_bits:expr, $num_bits:expr) => {
        mod $name {
            use super::*;

            mod from_string {
                use std::sync::LazyLock;
                use super::*;
    
                static REPORT:LazyLock<Report> = LazyLock::new(|| Report::try_from_report_str($report, $num_bits).unwrap());
    
                #[test]
                fn report_is_correct() {
                    assert_eq!(REPORT.report, $report);
                }
    
                #[test]
                fn packed_bits_is_correct() {
                    assert_eq!(REPORT.packed_bits, $packed_bits);
                }
            }

            mod from_packed_bits {
                use std::sync::LazyLock;
                use super::*;
    
                static REPORT:LazyLock<Report> = LazyLock::new(|| Report::try_from_packed_bits($packed_bits, $num_bits).unwrap());
    
                #[test]
                fn report_is_correct() {
                    assert_eq!(REPORT.report, $report);
                }
    
                #[test]
                fn packed_bits_is_correct() {
                    assert_eq!(REPORT.packed_bits, $packed_bits);
                }
            }
    
        }
    };}

    generate_tests!(signal_report_neg_30, "-30", 0b111111010010101, 15);
    generate_tests!(signal_report_pos_00, "+00", 0b111111010110011, 15);
    generate_tests!(signal_report_pos_50, "+50", 0b111111011100101, 15);
    generate_tests!(signal_report_neg_50, "-50", 0b111111011100110, 15);
    generate_tests!(signal_report_neg_31, "-31", 0b111111011111001, 15);
    generate_tests!(special_rrr, "RRR", 0b111111010010010, 15);
    generate_tests!(special_rr73, "RR73", 0b111111001110101, 15);
    generate_tests!(special_73, "73", 0b111111010010100, 15);
    generate_tests!(grid_aa00, "AA00", 0b000000000000000, 15);
    generate_tests!(grid_cn87, "CN87", 0b001001101111011, 15);
    generate_tests!(grid_rr99, "RR99", 0b111111010001111, 15);
    generate_tests!(grid_aa00aa, "AA00AA", 0b0000000000000000000000000, 25);
    generate_tests!(grid_aa00ab, "AA00AB", 0b0000000000000000000000001, 25);
    generate_tests!(grid_rr99rr, "RR99XX", 0b1000111001100001111111111, 25);

    //11110011000100000000000110100011101000110001000111001010101000000000010_01_0_100
    generate_tests!(special_rrr_2, "RRR", 0b01, 2);

    #[test]
    fn test_is_ack() {
        let r = Report::try_from_report_str("R+00", 15).unwrap();
        let r_str = format!("{}", r);
        assert_eq!(r_str, "R+00");
        assert_eq!(r.is_ack, true);
        assert_eq!(r.report, "+00");
    }

}