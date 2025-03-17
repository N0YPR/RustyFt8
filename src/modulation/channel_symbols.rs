use bitvec::prelude::*;
use crate::{constants::FT8_COSTAS, error_correction::gray::GrayCode};

pub fn channel_symbols(bits: &BitSlice) -> Vec<u8> {
    // convert the bits into 3 bit symbols
    let mut symbols: Vec<u8> = vec![];
    for chunk in bits.chunks_exact(3) {
        let value = chunk.load_be::<u8>() & 0b0000_0111;
        symbols.push(value);
    }

    // gray encode the symbols
    let gray = GrayCode::new();
    let gray_coded_symbols = gray.encode(&symbols);

    // add the costas arrays
    let mut channel_symbols: Vec<u8> = Vec::new();
    channel_symbols.extend(FT8_COSTAS);
    channel_symbols.extend_from_slice(&gray_coded_symbols[..29]);
    channel_symbols.extend(FT8_COSTAS);
    channel_symbols.extend_from_slice(&gray_coded_symbols[29..]);
    channel_symbols.extend(FT8_COSTAS);

    channel_symbols
}
