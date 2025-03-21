use bitvec::prelude::*;
use crate::{constants::FT8_COSTAS, error_correction::gray::encode};

use crate::util::bitvec_utils::FromBitSlice;

pub fn channel_symbols(bits: &BitSlice) -> Vec<u8> {
    // convert the bits into 3 bit symbols
    let mut symbols: Vec<u8> = vec![];
    for chunk in bits.chunks_exact(3) {
        let value = u8::from_bitslice(chunk);
        symbols.push(value);
    }

    // gray encode the symbols
    let gray_coded_symbols = encode(&symbols);

    // add the costas arrays
    let mut channel_symbols: Vec<u8> = Vec::new();
    channel_symbols.extend(FT8_COSTAS);
    channel_symbols.extend_from_slice(&gray_coded_symbols[..29]);
    channel_symbols.extend(FT8_COSTAS);
    channel_symbols.extend_from_slice(&gray_coded_symbols[29..]);
    channel_symbols.extend(FT8_COSTAS);

    channel_symbols
}

#[cfg(test)]
mod tests {
    use crate::util::bitvec_utils::PackBitvecFieldType;

    use super::*;

    #[test]
    fn channel_symbols_as_expected_from_wsjtx() {
        let expected_symbols_str = "3140652000671215006116571652175424753140652705022270444424051477565353273140652".to_string();

        let message = 0b00000000010111100101100110000000010100100110110011100110110001100111110011001u128;
        let crc = 0b11101111001110u16;
        let parity = 0b00100000011011011111000110110110110011110000100001110111111100101100010100010011111u128;

        let mut bv = BitVec::new();
        message.pack_into_bitvec(&mut bv, 77);
        crc.pack_into_bitvec(&mut bv, 14);
        parity.pack_into_bitvec(&mut bv, 83);

        let channel_symbols = channel_symbols(&bv);

        let channel_symbols_str: String = channel_symbols.iter()
            .map(|&symbol| symbol.to_string())
            .collect();
        
        assert_eq!(channel_symbols_str, expected_symbols_str);
        
    }
}