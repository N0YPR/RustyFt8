use bitvec::prelude::*;
use crate::{message::gray::GrayCode, util::bitvec_utils::PackBitvecFieldType};

pub fn channel_symbols(message: u128, crc: u16, parity: u128) -> Vec<u8> {
    let mut bv: BitVec<u8, Msb0> = BitVec::new();
    message.pack_into_bitvec(&mut bv, 77);
    crc.pack_into_bitvec(&mut bv, 14);
    parity.pack_into_bitvec(&mut bv, 83);
    // convert the bits into 3 bit symbols
    let mut symbols: Vec<u8> = vec![];
    for chunk in bv.chunks_exact(3) {
        let value = chunk.load_be::<u8>() & 0b0000_0111;
        symbols.push(value);
    }
    let gray = GrayCode::new();
    let gray_coded_symbols = gray.encode(&symbols);
    let costas: Vec<u8> = vec![3, 1, 4, 0, 6, 5, 2];
    let mut channel_symbols: Vec<u8> = Vec::new();
    channel_symbols.extend(costas.iter());
    channel_symbols.extend_from_slice(&gray_coded_symbols[..29]);
    channel_symbols.extend(costas.iter());
    channel_symbols.extend_from_slice(&gray_coded_symbols[29..]);
    channel_symbols.extend(costas.iter());

    channel_symbols
}
