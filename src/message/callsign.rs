use std::fmt::Display;
use std::sync::Mutex;
use lru::LruCache;
use lazy_static::lazy_static;
use snafu::prelude::*;
use crate::constants::*;

use super::radix::ToStrMixedRadix;
use super::radix::FromMixedRadixStr;

lazy_static! {
    static ref CALLSIGN_CACHE: Mutex<LruCache<u32, String>> = Mutex::new(LruCache::new(10000));
}

#[derive(Debug)]
pub struct Callsign {
    pub callsign: String,
    pub is_rover: bool,
    pub is_portable: bool,
    pub is_hashed: bool,
    pub was_hashed: bool,
    pub packed_58bits: u64,
    pub packed_28bits: u32,
    pub hashed_22bits: u32,
    pub hashed_12bits: u32,
    pub hashed_10bits: u32
}

impl Display for Callsign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.callsign, if self.is_rover {"/R"} else if self.is_portable {"/P"} else {""})
    }
}

impl TryFrom<u32> for Callsign {
   type Error = ParseCallsignError;
   
   fn try_from(value: u32) -> Result<Self, Self::Error> {
        if value == 0 {
            return Callsign::from_callsign_str("DE");
        }

        if value == 1 {
            return Callsign::from_callsign_str("QRZ");
        }

        if value == 2 {
            return Callsign::from_callsign_str("CQ");
        }

        if value >= 3 && value <= 1002 {
            return Callsign::from_callsign_str(&format!("CQ {:0>3}", value - 3));
        }

        if value >= 1004 && value <= 532443 {
            let radix_tables = [
                FT8_CHAR_TABLE_ALPHA_SPACE,
                FT8_CHAR_TABLE_ALPHA_SPACE,
                FT8_CHAR_TABLE_ALPHA_SPACE,
                FT8_CHAR_TABLE_ALPHA_SPACE,
            ];
            let s = match (value - 1003).to_str_mixed_radix(&radix_tables) {
                Ok(value) => value,
                Err(_) => {
                    return Err(ParseCallsignError::OutOfRange);
                }
            };
            return Callsign::from_callsign_str(&format!("CQ {}", s.trim()));
        }

        if value >= 2063592 && value <= 6257895 {
            if let Some(c) = get_hashed_callsign_string(value) {
                return Callsign::from_callsign_str(&c);
            }
            let c = Callsign{
                callsign : "...".to_string(),
                is_rover: false,
                is_portable: false,
                is_hashed: true,
                was_hashed: false,
                packed_58bits : 0,
                packed_28bits : value,
                hashed_22bits : (value - 2063592), 
                hashed_12bits : (value - 2063592) >> 10, 
                hashed_10bits : (value - 2063592) >>  12};
            return Ok(c);
        }

        if value >= 6257896 && value <= 274693351{
            let radix_tables = [
                FT8_CHAR_TABLE_ALPHANUM_SPACE,
                FT8_CHAR_TABLE_ALPHANUM,
                FT8_CHAR_TABLE_NUMERIC,
                FT8_CHAR_TABLE_ALPHA_SPACE,
                FT8_CHAR_TABLE_ALPHA_SPACE,
                FT8_CHAR_TABLE_ALPHA_SPACE
            ];
            let mut s = match (value - 6257896).to_str_mixed_radix(&radix_tables) {
                Ok(value) => value,
                Err(_) => {
                    return Err(ParseCallsignError::OutOfRange);
                }
            };
            
            // special case for 3DA0...
            if s.starts_with("3D0") {
                s = s.replacen("3D0","3DA0", 1,);
            }

            // special case for 3XB...
            if s.starts_with("Q") {
                s = s.replacen("Q", "3X", 1);
            }
           

            return Callsign::from_callsign_str(s.trim());
        }

        return Err(ParseCallsignError::OutOfRange);
    }
   
}

impl TryFrom <u64> for Callsign {
    type Error = ParseCallsignError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let radix_table = [
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH,
            FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH
        ];
        let s = match value.to_str_mixed_radix(&radix_table) {
            Ok(value) => value,
            Err(_) => {
                return Err(ParseCallsignError::OutOfRange);
            }
        };

        return Callsign::from_callsign_str(s.trim());
    }
}

impl Callsign {
    pub fn to_string(&self) -> String {
        format!("{}{}", self.callsign, if self.is_rover {"/R"} else if self.is_portable {"/P"} else {""})
    }

    pub fn try_from_callsign_hash(hash:u32) -> Result<Self, ParseCallsignError> {
        if let Some(callsign) = get_hashed_callsign_string(hash) {
            return Callsign::from_callsign_str(&callsign);
        }

        let callsign = Callsign{
            callsign : "<...>".to_string(),
            is_rover: false,
            is_portable: false,
            is_hashed: true,
            was_hashed: false,
            packed_58bits : 0,
            packed_28bits : 0, 
            hashed_22bits : 0, 
            hashed_12bits : 0, 
            hashed_10bits : 0};

        return Ok(callsign);
    }

    pub fn from_callsign_str(s: &str) -> Result<Self, ParseCallsignError> {
        let is_rover = s.ends_with("/R");
        let is_portable = s.ends_with("/P");
        let was_hashed:bool;

        let mut string_to_pack = if is_rover || is_portable { &s[0..s.len()-2]} else {s};
        if string_to_pack.starts_with("<") && string_to_pack.ends_with(">") {
            string_to_pack = rem_first_and_last(&string_to_pack);
            was_hashed = true;
        } else {
            was_hashed = false;
        }

        let packed28 = match pack_callsign_into_28bits(string_to_pack) {
            Ok(value) => value,
            Err(err) => {
                return Err(err);
            }
        };

        let is_hashed = packed28 >= 2063592 && packed28 <= 6257895;

        let packed58 = match pack_callsign_into_58bits(string_to_pack){
            Ok(value) => value,
            Err(err) => {
                return Err(err);
            }
        };
        
        let hashed22 = hash_callsign(string_to_pack, 22) as u32;
        let hashed12 = hash_callsign(string_to_pack, 12) as u32;
        let hashed10 = hash_callsign(string_to_pack, 10) as u32;

        store_hashed_callsign_string(packed28, string_to_pack.to_string());
        store_hashed_callsign_string(hashed10, string_to_pack.to_string());
        store_hashed_callsign_string(hashed12, string_to_pack.to_string());
        store_hashed_callsign_string(hashed22, string_to_pack.to_string());

        //let packed_callsign = format!("{}{}", string_to_pack, if is_rover {"/R"} else if is_portable {"/P"} else {""});
        let packed_callsign = format!("{}", string_to_pack);

        let c = Callsign{
            callsign: packed_callsign,
            is_rover,
            is_portable,
            is_hashed,
            was_hashed,
            packed_58bits : packed58,
            packed_28bits : packed28,
            hashed_22bits : hashed22, 
            hashed_12bits : hashed12, 
            hashed_10bits : hashed10 };

        return Ok(c);
    }
}

#[derive(Debug, Snafu)]
pub enum ParseCallsignError {
    /// String is not a valid length to be a callsign
    #[snafu(display("invalid length string"))]
    InvalidLength,

    /// String contains invalid character
    /// 
    /// Must be in " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/"
    #[snafu(display("string contains invalid character"))]
    InvalidChar,

    /// Integer out of range to be a valid callsign
    #[snafu(display("integer out of range"))]
    OutOfRange
}

pub fn pack_callsign_into_28bits(callsign:&str) -> Result<u32, ParseCallsignError> {
    if callsign.len() < 2 || callsign.len() > 11 {
        return Err(ParseCallsignError::InvalidLength)
    }

    // special tokens see https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf table 7 on page 16
    // DE 0
    if callsign == "DE" {
        return Ok(0);
    }

    // QRZ 1
    if callsign == "QRZ" { 
        return Ok(1);    
    }

    // CQ 2
    if callsign == "CQ"{
        return Ok(2);
    }

    // CQ 000 - CQ 999 3 to 1002
    if callsign.starts_with("CQ ") {
        let remainder = &callsign[3..];
        if remainder.len() == 3 && remainder.chars().all(char::is_numeric) {
            match u32::from_mixed_radix_str(
                remainder,
                &[
                    FT8_CHAR_TABLE_NUMERIC,
                    FT8_CHAR_TABLE_NUMERIC,
                    FT8_CHAR_TABLE_NUMERIC
                ]) {
                    Ok(value) => {
                        return Ok(value + 3);
                    },
                    Err(_) => {}
                };
        }
    }

    // CQ A - CQ Z 1004 to 1029
    // CQ AA - CQ ZZ 1031 to 1731
    // CQ AAA - CQ ZZZ 1760 to 20685
    // CQ AAAA - CQ ZZZZ 21443 to 532443
    if callsign.starts_with("CQ ") {
        let remainder = &callsign[3..];
        if remainder.len() >= 1 && remainder.len() <= 4 && remainder.chars().all(|c| FT8_CHAR_TABLE_ALPHA_SPACE.contains(c)) {
            let padded_remainder = format!("{: >4}", remainder);
            match u32::from_mixed_radix_str(
                &padded_remainder,
                &[
                    FT8_CHAR_TABLE_ALPHA_SPACE,
                    FT8_CHAR_TABLE_ALPHA_SPACE,
                    FT8_CHAR_TABLE_ALPHA_SPACE,
                    FT8_CHAR_TABLE_ALPHA_SPACE,
            ]) {
                Ok(value) => { return Ok(value + 1003); },
                Err(_) => {}
            };
        }
    }

    // CQ anything else is error
    if callsign.starts_with("CQ ") {
        return Err(ParseCallsignError::InvalidChar);
    }

    let mut adjusted_callsign = callsign.to_owned();

    // Workaround for Swaziland prefix
    if adjusted_callsign.starts_with("3DA0") {
        adjusted_callsign = callsign.replacen("3DA0","3D0", 1,);
    }

    // Workaround for Guinea prefix
    if adjusted_callsign.starts_with("3X") 
    && adjusted_callsign.chars().nth(2).unwrap() >= 'B' 
    && adjusted_callsign.chars().nth(2).unwrap() <= 'Z' {
        let lastn:String = adjusted_callsign.chars().skip(2).take(adjusted_callsign.len()-2).collect();
        adjusted_callsign = format!("Q{}", lastn);
    }

    if !adjusted_callsign.chars().all(|c| FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH.contains(c)) {
        return Err(ParseCallsignError::InvalidChar);
    }

    // might be a standard callsign, attempt to align it and parse
    // Standard call signs 6257896 + (0 to 268435455)
    let aligned_callsign = align_callsign(&adjusted_callsign);
    match u32::from_mixed_radix_str(
        &aligned_callsign,
        &[
            FT8_CHAR_TABLE_ALPHANUM_SPACE,
            FT8_CHAR_TABLE_ALPHANUM,
            FT8_CHAR_TABLE_NUMERIC,
            FT8_CHAR_TABLE_ALPHA_SPACE,
            FT8_CHAR_TABLE_ALPHA_SPACE,
            FT8_CHAR_TABLE_ALPHA_SPACE
    ]) {
        Ok(value) => { return Ok(value + 6257896)},
        Err(_) => {}
    };
    
    // must be a non-standard callsign, return a 22 bit hash
    // 22-bit hash codes 2063592 + (0 to 4194303)
    //let hash = (hash_callsign(&aligned_callsign) >> 42) as u32;
    let hash = hash_callsign(&adjusted_callsign, 22) as u32;
    return Ok(2063592 + hash);
}

fn rem_first_and_last(value: &str) -> &str {
    let mut chars = value.chars();
    chars.next();
    chars.next_back();
    chars.as_str()
}

fn pack_callsign_into_58bits(callsign:&str) -> Result<u64, ParseCallsignError> {
    if callsign.len() == 0 || callsign.len() > 11 {
        return Err(ParseCallsignError::InvalidLength)
    }

    if !callsign.chars().all(|c| FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH.contains(c)) {
        return Err(ParseCallsignError::InvalidChar);
    }

    let right_aligned_callsign = format!("{: >11}", callsign);

    let l = FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH.len() as u64;
    let mut value:u64 = 0;
    for c in right_aligned_callsign.chars() {
        let pos = FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH.chars().position(|ch| c == ch ).unwrap() as u64;
        value = value * l + pos;
    }
    return Ok(value);
}

fn index_of_last_number(s:&str) -> Option<usize> {
    for index in (0..s.len()-1).rev() {
        let c = s.chars().nth(index);
        if c.is_some_and(|c| c.is_numeric()) {
            return Some(index);
        }
    }
    return None;
}

fn align_callsign(callsign:&str) -> String {
    // Align the callsign into a 6-character field by identifying the last 
    // (or only) digit, and placing it in the third position. If there are 
    // fewer than two characters before the digit, or fewer than three 
    // characters after the digit, pad with spaces.

    let separating_numeral_index = index_of_last_number(callsign);

    // non-standard callsign format
    if separating_numeral_index.is_none() {
        return String::from(callsign);
    }

    // prefix is all the characters before the separating numeral
    let prefix = &callsign[0..separating_numeral_index.unwrap()];

    // get the separating numeral itself
    let separating_numeral = callsign.chars().nth(separating_numeral_index.unwrap()).unwrap();

    // suffix is all the characters after the separating numeral
    let suffix = &callsign[separating_numeral_index.unwrap() + 1..];

    // format with padding
    return format!("{: >2}{}{: <3}", prefix, separating_numeral, suffix);
}

fn hash_callsign(callsign:&str, m:u32) -> u64 {
    let left_aligned_callsign = format!("{: <11}", callsign);

    let l = FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH.len() as u64;
    let mut value:u64 = 0;
    for c in left_aligned_callsign.chars() {
        let pos = FT8_CHAR_TABLE_ALPHANUM_SPACE_SLASH.chars().position(|ch| c == ch ).unwrap() as u64;
        value = value * l + pos;
    }

    value = (value as u128 * 47055833459u128) as u64 >> (64-m);
    return value;
}

fn store_hashed_callsign_string(hash:u32, callsign:String) {
    let mut cache = CALLSIGN_CACHE.lock().unwrap();
    cache.put(hash, callsign);
}

fn get_hashed_callsign_string(hash:u32) -> Option<String> {
    let mut cache = CALLSIGN_CACHE.lock().unwrap();
    if let Some(callsign) = cache.get(&hash) {
        return Some(callsign.clone());
    }
    None
}

#[cfg(test)]
mod tests {

    use super::*;


    macro_rules! test_callsign_success {
        ($name:ident, $callsign:expr, $expected_callsign:expr, $expected_rover:expr) => {
            paste::item! {
                #[test]
                fn callsign_should_be_expected() {
                    let callsign = format!("{}", Callsign::from_callsign_str($callsign).unwrap());
                    assert_eq!(callsign, $expected_callsign);
                }

                #[test]
                fn rover_should_be_expected() {
                    let callsign = Callsign::from_callsign_str($callsign).unwrap();
                    assert_eq!(callsign.is_rover, $expected_rover);
                }
            }
        };
    }

    macro_rules! test_28bits_success {
        ($name:ident, $callsign:expr, $packed28:expr) => {
            paste::item! {
                #[test]
                fn [< packed_28bits_should_be_ $packed28 >]() {
                    let callsign = Callsign::from_callsign_str($callsign).unwrap();
                    assert_eq!(callsign.packed_28bits, $packed28);
                }
            }
        };
    }

    macro_rules! test_standard_callsign_success {
        ($name:ident, $callsign:expr, $expected_callsign:expr, $expected_rover:expr, $packed28:expr) => {
            paste::item! {
                mod [< with_ $name:lower >] {
                    use super::*;
                    test_callsign_success!($name, $callsign, $expected_callsign, $expected_rover);
                    test_28bits_success!($name, $callsign, $packed28);
                }
            }
        };
    }

    macro_rules! test_nonstd_callsign_success {
        ($name:ident, $callsign:expr, $expected_callsign:expr, $expected_rover:expr, $packed28:expr, $packed58:expr, $hashed22:expr, $hashed12:expr, $hashed10:expr) => {
            paste::item! {
                mod [< with_ $name:lower >] {
                    use super::*;

                    test_callsign_success!($name, $callsign, $expected_callsign, $expected_rover);
                    test_28bits_success!($name, $callsign, $packed28);

                    #[test]
                    fn [< packed_58bits_should_be_ $packed58 >]() {
                        let callsign = Callsign::from_callsign_str($callsign).unwrap();
                        assert_eq!(callsign.packed_58bits, $packed58);
                    }

                    #[test]
                    fn [< hashed_22bits_should_be_ $hashed22 >]() {
                        let callsign = Callsign::from_callsign_str($callsign).unwrap();
                        assert_eq!(callsign.hashed_22bits, $hashed22);
                    }

                    #[test]
                    fn [< hashed_12bits_should_be_ $hashed12 >]() {
                        let callsign = Callsign::from_callsign_str($callsign).unwrap();
                        assert_eq!(callsign.hashed_12bits, $hashed12);
                    }

                    #[test]
                    fn [< hashed_10bits_should_be_ $hashed10 >]() {
                        let callsign = Callsign::from_callsign_str($callsign).unwrap();
                        assert_eq!(callsign.hashed_10bits, $hashed10);
                    }
                }
            }
        };
    }

    mod callsign_from_str {
        use super::*;
        test_nonstd_callsign_success!(n0ypr, "N0YPR", "N0YPR", false, 10803661, 50149692, 1836698, 1793, 448);
        test_nonstd_callsign_success!(ve5_slant_n0ypr, "VE5/N0YPR", "VE5/N0YPR", false, 5686519, 140866629639964, 3622927, 3538, 884);
        test_nonstd_callsign_success!(bracket_ve5_slant_n0ypr, "<VE5/N0YPR>", "VE5/N0YPR", false, 5686519, 140866629639964, 3622927, 3538, 884);
        test_standard_callsign_success!(de, "DE", "DE",false, 0);
        test_standard_callsign_success!(qrz, "QRZ", "QRZ",false, 1);
        test_standard_callsign_success!(n0ypr_slant_r, "N0YPR/R", "N0YPR/R", true, 10803661);
        test_standard_callsign_success!(cq_000, "CQ 000", "CQ 000",false, 3);
        test_standard_callsign_success!(cq_001, "CQ 001", "CQ 001",false, 4);
        test_standard_callsign_success!(cq_999, "CQ 999", "CQ 999",false, 1002);
        test_standard_callsign_success!(cq_a, "CQ A", "CQ A",false, 1004);
        test_standard_callsign_success!(cq_b, "CQ B", "CQ B",false, 1005);
        test_standard_callsign_success!(cq_z, "CQ Z", "CQ Z",false, 1029);
        test_standard_callsign_success!(cq_aa, "CQ AA", "CQ AA",false, 1031);
        test_standard_callsign_success!(cq_ab, "CQ AB", "CQ AB",false, 1032);
        test_standard_callsign_success!(cq_zz, "CQ ZZ", "CQ ZZ",false, 1731);
        test_standard_callsign_success!(cq_aaa, "CQ AAA", "CQ AAA",false, 1760);
        test_standard_callsign_success!(cq_aab, "CQ AAB", "CQ AAB",false, 1761);
        test_standard_callsign_success!(cq_zzz, "CQ ZZZ", "CQ ZZZ",false, 20685);
        test_standard_callsign_success!(cq_aaaa, "CQ AAAA", "CQ AAAA",false, 21443);
        test_standard_callsign_success!(cq_aaab, "CQ AAAB", "CQ AAAB",false, 21444);
        test_standard_callsign_success!(cq_zzzz, "CQ ZZZZ", "CQ ZZZZ",false, 532443);
        test_standard_callsign_success!(n0ypr_bracket, "<N0YPR>", "N0YPR",false, 10803661);
       
        macro_rules! test_pack28bits_error {
            ($name:ident, $callsign:expr, $expectederror:expr) => {
                paste::item! {
                    #[test]
                    fn [< with_ $name:lower >]() {
                        assert!(matches!(pack_callsign_into_28bits($callsign), Err($expectederror)));
                    }
                }
            };
        }
        test_pack28bits_error!(disallowed_chars_should_return_invalid_char_error, "***", ParseCallsignError::InvalidChar);
        test_pack28bits_error!(too_many_chars_should_return_invalid_length_error, "ABCDEFGHIJKL", ParseCallsignError::InvalidLength);
        test_pack28bits_error!(empty_string_should_return_invalid_length_error, "", ParseCallsignError::InvalidLength);
    }

    mod callsign_try_from {
        use super::*;

        macro_rules! test_from_value_succeeds {
            ($name:ident, $value:expr, $expected:expr) => {
                #[test]
                fn $name() {
                    assert_eq!(Callsign::try_from($value).unwrap().callsign, $expected);
                }
            };
        }

        test_from_value_succeeds!(with_0_returns_de, 0u32, "DE");
        test_from_value_succeeds!(with_1_returns_qrz, 1u32, "QRZ");
        test_from_value_succeeds!(with_2_returns_cq, 2u32, "CQ");
        test_from_value_succeeds!(with_3_returns_cq_000, 3u32, "CQ 000");
        test_from_value_succeeds!(with_1002_returns_cq_999, 1002u32, "CQ 999");
        test_from_value_succeeds!(with_1004_returns_cq_a, 1004u32, "CQ A");
        test_from_value_succeeds!(with_1029_returns_cq_z, 1029u32, "CQ Z");
        test_from_value_succeeds!(with_1031_returns_cq_aa, 1031u32, "CQ AA");
        test_from_value_succeeds!(with_1731_returns_cq_zz, 1731u32, "CQ ZZ");
        test_from_value_succeeds!(with_1760_returns_cq_aaa, 1760u32, "CQ AAA");
        test_from_value_succeeds!(with_20685_returns_cq_zzz, 20685u32, "CQ ZZZ");
        test_from_value_succeeds!(with_21443_returns_cq_aaaa, 21443u32, "CQ AAAA");
        test_from_value_succeeds!(with_532443_returns_cq_zzzz, 532443u32, "CQ ZZZZ");
        test_from_value_succeeds!(with_10803661_returns_n0ypr, 10803661u32, "N0YPR");
        test_from_value_succeeds!(with_268435455_returns_zz9zzz, 268435455u32, "ZZ9ZZZ");
        test_from_value_succeeds!(with_199919690_returns_3xb9aaa, 199919690u32, "3XB9AAA");

        #[test]
        fn with_10803661_returns_is_hashed_false() {
            let callsign = Callsign::try_from(10803661u32).unwrap();
            assert!(!callsign.is_hashed);
        }

        mod with_104568930255160u64 {
            use super::*;

            #[test]
            fn returns_is_hashed_true() {
                let callsign = Callsign::try_from(104568930255160u64).unwrap();
                assert!(callsign.is_hashed);
            }

            #[test]
            fn returns_callsign_n0yprslantve5() {
                let callsign = Callsign::try_from(104568930255160u64).unwrap();
                assert_eq!(callsign.callsign, "N0YPR/VE5");
            }
        }

        //n0ypr_slant_ve5, "N0YPR/VE5", 2386265
        mod with_2386265 {
            use super::*;

            #[test]
            fn is_hashed_is_true() {
                let callsign = Callsign::try_from(2386265u32).unwrap();
                assert_eq!(callsign.is_hashed, true);
            }

            #[test]
            fn packed_28_is_2386265() {
                let callsign = Callsign::try_from(2386265u32).unwrap();
                assert_eq!(callsign.packed_28bits, 2386265);
            }

            #[test]
            fn hash22_is_322673() {
                let callsign = Callsign::try_from(2386265u32).unwrap();
                assert_eq!(callsign.hashed_22bits, 322673);
            }

            #[test]
            fn hash12_is_315() {
                let callsign = Callsign::try_from(2386265u32).unwrap();
                assert_eq!(callsign.hashed_12bits, 315);
            }

            #[test]
            fn hash10_is_78() {
                let callsign = Callsign::try_from(2386265u32).unwrap();
                assert_eq!(callsign.hashed_10bits, 78);
            }

        
        }

        #[test]
        fn with_268435456_returns_out_of_range() {
            let result = Callsign::try_from(268435456u32);
            assert!(matches!(result, Err(ParseCallsignError::OutOfRange)));
        }
    }

    mod cache {
        use crate::message::callsign::Callsign;

        #[test]
        fn can_cache() {
            let callsign1 = Callsign::from_callsign_str("PJ4/K1ABC").expect("callsign should have been cached");

            let h10 = callsign1.hashed_10bits;
            let h12 = callsign1.hashed_12bits;
            let h22 = callsign1.hashed_22bits;

            assert_eq!("PJ4/K1ABC", Callsign::try_from_callsign_hash(h10).expect("callsign should have been cached").callsign);
            assert_eq!("PJ4/K1ABC", Callsign::try_from_callsign_hash(h12).expect("callsign should have been cached").callsign);
            assert_eq!("PJ4/K1ABC", Callsign::try_from_callsign_hash(h22).expect("callsign should have been cached").callsign);
        }
    }

    mod cached_c28 {
        use super::*;

        #[test]
        fn can_deal_with_hashed() {
            let callsign1 = Callsign::from_callsign_str("PJ4/K1ABC").expect("callsign should have parsed");
            let callsign2 = Callsign::try_from(callsign1.packed_28bits) .expect("callsign should have parsed");
            println!("callsign1: {:?}", callsign1);
            println!("callsign2: {:?}", callsign2);
            assert_eq!(callsign1.packed_28bits, callsign2.packed_28bits);
            assert_eq!(callsign1.callsign, callsign2.callsign);
        }
    }
}