use std::collections::HashMap;

use crate::constants::FT8_GRAY_CODE;

pub struct GrayCode {
    encoder: HashMap<u8, u8>,
    decoder: HashMap<u8, u8>
}

impl GrayCode {
    pub fn new() -> Self {
        let mut encoder: HashMap<u8,u8> = HashMap::new();
        let mut decoder: HashMap<u8,u8> = HashMap::new();

        for i in 0..FT8_GRAY_CODE.len() {
            let symbol = i as u8;
            let encoded_symbol = FT8_GRAY_CODE[i];
            encoder.insert(symbol, encoded_symbol);
            decoder.insert(encoded_symbol, symbol);
        }

        GrayCode {
            encoder,
            decoder
        }
    }

    pub fn encode(&self, symbols: &[u8]) -> Vec<u8> {
        let mut encoded = vec![];
        for symbol in symbols {
            let encoded_symbol = *self.encoder.get(symbol).unwrap();
            encoded.push(encoded_symbol);
        }
        let encoded = encoded;
        encoded
    }

    pub fn decode(&self, symbols: &[u8]) -> Vec<u8> {
        let mut decoded = vec![];
        for symbol in symbols {
            let decoded_symbol = *self.decoder.get(symbol).unwrap();
            decoded.push(decoded_symbol);
        }
        let decoded = decoded;
        decoded
    }
}

mod tests {
    use crate::message::gray::GrayCode;

    #[test]
    fn test_gray_encoding() {
        let symbols:Vec<u8> = vec![0,1,2,3,4,5,6,7];
        let expected:Vec<u8> = vec![0,1,3,2,5,6,4,7];

        let encoder = GrayCode::new();
        let gray_encoded = encoder.encode(&symbols);

        assert_eq!(expected, gray_encoded);
    }

    #[test]
    fn encode_and_decode() {
        let symbols:Vec<u8> = vec![7, 0, 2, 7, 4, 1, 3, 2, 3, 6, 4, 1, 0, 0, 7, 6, 0, 2, 4, 1, 4, 3, 5, 3, 5, 3, 2, 4, 2, 1, 1, 6, 3, 7, 4, 6, 4, 0, 2, 7, 7, 3, 5, 6, 4, 2, 2, 5, 4, 3, 0, 0, 0, 2, 5, 3, 0, 1];
        let encoder = GrayCode::new();
        let gray_encoded = encoder.encode(&symbols);
        let gray_decoded = encoder.decode(&gray_encoded);
        assert_eq!(gray_decoded, symbols);
    }    
}
