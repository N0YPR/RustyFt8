#![allow(unused)]

use crate::constants::{FT8_GRAY_DECODE, FT8_GRAY_ENCODE};

pub fn encode(symbols: &[u8]) -> Vec<u8> {
    let mut encoded = vec![];
    for symbol in symbols {
        let encoded_symbol = FT8_GRAY_ENCODE[*symbol as usize];
        encoded.push(encoded_symbol);
    }
    let encoded = encoded;
    encoded
}

pub fn decode(symbols: &[u8]) -> Vec<u8> {
    let mut decoded = vec![];
    for symbol in symbols {
        let decoded_symbol = FT8_GRAY_DECODE[*symbol as usize];
        decoded.push(decoded_symbol);
    }
    let decoded = decoded;
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gray_encoding() {
        let symbols: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let expected: Vec<u8> = vec![0, 1, 3, 2, 5, 6, 4, 7];

        let gray_encoded = encode(&symbols);

        assert_eq!(expected, gray_encoded);
    }

    #[test]
    fn encode_and_decode() {
        let symbols: Vec<u8> = vec![
            7, 0, 2, 7, 4, 1, 3, 2, 3, 6, 4, 1, 0, 0, 7, 6, 0, 2, 4, 1, 4, 3, 5, 3, 5, 3, 2, 4, 2,
            1, 1, 6, 3, 7, 4, 6, 4, 0, 2, 7, 7, 3, 5, 6, 4, 2, 2, 5, 4, 3, 0, 0, 0, 2, 5, 3, 0, 1,
        ];
        let gray_encoded = encode(&symbols);
        let gray_decoded = decode(&gray_encoded);
        assert_eq!(gray_decoded, symbols);
    }
}
