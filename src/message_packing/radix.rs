use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum ParseRadixStringError {
    /// Empty input string
    #[snafu(display("Empty input string"))]
    EmptyString,

    /// Invalid radix table
    #[snafu(display("Invalid radix table"))]
    InvalidRadixTable,

    /// Length of input string and radix table length must match
    #[snafu(display("Length of input string and radix table length must match"))]
    LengthMismatch,

    /// Input string contains an invalid character
    #[snafu(display("Input string contains an invalid character"))]
    InvalidChar,

    /// Input is out of range
    #[snafu(display("Input is out of range"))]
    InputOutOfRange,
}

pub trait FromStrCustomRadix {
    type Item;

    fn from_str_custom_radix(input:&str, radix_table:&str) -> Result<Self::Item, ParseRadixStringError>;
}

macro_rules! from_str_custom_radix_impl {
    ($($t:ty)*) => {$(
        impl FromStrCustomRadix for $t {
            type Item = $t;

            fn from_str_custom_radix(input:&str, radix_table:&str) -> Result<Self::Item, ParseRadixStringError> {
                if input.len() == 0 {
                    return Err(ParseRadixStringError::EmptyString);
                }
                let radix_length = radix_table.len() as Self::Item;
                if radix_length == 0 {
                    return Err(ParseRadixStringError::InvalidRadixTable);
                }
                let mut value:Self::Item = 0;
                for c in input.chars() {
                    if let Some(pos) = radix_table.chars().position(|ch| c == ch) {
                        value = value * radix_length + pos as Self::Item;
                    } else {
                        return Err(ParseRadixStringError::InvalidChar);
                    }
                }

                Ok(value)
            }
        }
    )*}
}
from_str_custom_radix_impl!(u32 u64 u128);

pub trait FromMixedRadixStr {
    type Item;
    fn from_mixed_radix_str(input:&str, radix_tables:&[&str]) -> Result<Self::Item, ParseRadixStringError>;
}
macro_rules! from_mixed_radix_str_impl {
    ($($t:ty)*) => {$(
        impl FromMixedRadixStr for $t {
            type Item = $t;
            
            fn from_mixed_radix_str(input:&str, radix_tables:&[&str]) -> Result<Self::Item, ParseRadixStringError> {
        
                if input.len() == 0 {
                    return Err(ParseRadixStringError::EmptyString);
                }
        
                if input.len() > radix_tables.len() {
                    return Err(ParseRadixStringError::InvalidRadixTable);
                }
        
                // measure the length of each of the radix tables
                let table_sizes:Vec<Self::Item> = radix_tables.iter().map(|t| t.len() as Self::Item).collect();
        
                // start at 0
                let mut value:Self::Item = 0;
        
                // enumerate through all the characters of the input
                for (i, c) in input.chars().enumerate() {
                    // find the position of the char in its cooresponding radix table
                    let position = match radix_tables[i].chars().position(|ch| ch == c) {
                        Some(value) => value,
                        None => return Err(ParseRadixStringError::InvalidChar)
                    };
        
                    // the value for the digit starts with the position
                    let mut position_value = position as Self::Item;
        
                    // then multiply by all the other radix lengths to the right
                    for size in &table_sizes[i+1..]{
                        position_value *= size;
                    }
        
                    // add to the value
                    value += position_value;
                }
        
                return Ok(value);
            }
        }
    )*}
}
from_mixed_radix_str_impl!(u32 u64 u128);

pub trait ToStrMixedRadix {
    type Item;

    fn to_str_mixed_radix(&self, radix_tables:&[&str]) -> Result<String, ParseRadixStringError>;
}

macro_rules! to_str_mixed_radix_impl {
    ($($t:ty)*) => {$(
        impl ToStrMixedRadix for $t {
            type Item = $t;

            fn to_str_mixed_radix(&self, radix_tables:&[&str]) -> Result<String, ParseRadixStringError> {
                // measure the length of each of the radix tables
                let table_sizes:Vec<Self::Item> = radix_tables.iter().map(|t| t.len() as Self::Item).collect();

                let mut vec:Vec<char> = vec![];
                let mut current_value:Self::Item = *self;

                // enumerate through all the radix tables
                for (i, t) in radix_tables.iter().enumerate() {
                    // calculate the radix factor
                    let mut radix_factor:Self::Item = 1;
                    for size in &table_sizes[i+1..]{
                        radix_factor *= size;
                    }

                    let position_value = current_value / radix_factor;
                    current_value = current_value - radix_factor * position_value;

                    // look up the character from the table and append it to the string
                    match t.chars().nth(position_value as usize) {
                        Some(v) => {
                            vec.push(v);
                        },
                        None => {
                            return Err(ParseRadixStringError::InputOutOfRange)
                        }
                    }


                }

                return Ok(vec.into_iter().collect());
            }
        }
    )*}
}
to_str_mixed_radix_impl!(u32 u64 u128);

#[cfg(test)]
mod tests {
    use crate::message_packing::constants::*;

    use super::*;

    mod from_str_custom_radix {
        use super::*;

        #[test]
        fn empty_input_string_returns_error() {
            assert!(matches!(u32::from_str_custom_radix("", FT8_CHAR_TABLE_FULL), Err(ParseRadixStringError::EmptyString)));
        }

        #[test]
        fn empty_radix_table_returns_error() {
            assert!(matches!(u32::from_str_custom_radix("TEST", ""), Err(ParseRadixStringError::InvalidRadixTable)));
        }

        #[test]
        fn input_string_with_invalid_char_returns_error() {
            assert!(matches!(u32::from_str_custom_radix("TEST", "01"), Err(ParseRadixStringError::InvalidChar)));
        }

        #[test]
        fn valid_input_string_binary_radix() {
            assert!(matches!(u32::from_str_custom_radix("  ", " A"), Ok(0)));
            assert!(matches!(u32::from_str_custom_radix(" A", " A"), Ok(1)));
            assert!(matches!(u32::from_str_custom_radix("A ", " A"), Ok(2)));
            assert!(matches!(u32::from_str_custom_radix("AA", " A"), Ok(3)));
        }

        #[test]
        fn valid_input_string_ternary_radix() {
            assert!(matches!(u32::from_str_custom_radix("  ", " AB"), Ok(0)));
            assert!(matches!(u32::from_str_custom_radix(" A", " AB"), Ok(1)));
            assert!(matches!(u32::from_str_custom_radix(" B", " AB"), Ok(2)));
            assert!(matches!(u32::from_str_custom_radix("A ", " AB"), Ok(3)));
            assert!(matches!(u32::from_str_custom_radix("AA", " AB"), Ok(4)));
            assert!(matches!(u32::from_str_custom_radix("AB", " AB"), Ok(5)));
            assert!(matches!(u32::from_str_custom_radix("B ", " AB"), Ok(6)));
            assert!(matches!(u32::from_str_custom_radix("BA", " AB"), Ok(7)));
            assert!(matches!(u32::from_str_custom_radix("BB", " AB"), Ok(8)));
        }

        #[test]
        fn valid_input_string_wsjtx_free_text() {
            // build/wsjtx-prefix/src/wsjtx-build/ft8sim "TEST" 1500 0 0 0 1 -10

            //     Decoded message: TEST                                    i3.n3: 0.0
            // f0: 1500.000   DT:  0.00   TxT:  12.6   SNR: -10.0  BW:50.0

            // Message bits: 
            // 00000000000000000000000000000000000000000000000001000100101011001101100000000
            // drop last 6 bits since those are the i3.n3
            // 1000100101011001101100

            assert!(matches!(u32::from_str_custom_radix("TEST", FT8_CHAR_TABLE_FULL), Ok(0b1000100101011001101100)));
        }
    }
}