use std::vec;

use bitvec::prelude::*;
use encode::{gray::{GrayCode, FT8_GRAY_CODE}, ldpc::{Ldpc, FT8_GENERATOR_HEX_STRINGS}};
use message::message::Message;

mod message;
mod encode;

fn main() {
    println!("Hello, world!");

    let message = Message::try_from("CQ N0YPR/R DM42").unwrap();
    println!("Message: {}", message);

    println!("Message bits: {:077b}", message.bits());

    println!("Checksum: {:014b}", message.checksum());

    let message_plus_checksum = (message.bits() << 14) | message.checksum() as u128;
    println!("Message & Checksum: {:091b}", message_plus_checksum);

    let ldpc = Ldpc::new(&FT8_GENERATOR_HEX_STRINGS);
    let parity = ldpc.generate_parity(&message_plus_checksum);

    println!("Parity: {:083b}", parity);

    let mut bits = BitVec::<u64, Msb0>::new();

    // push the 77 bits of message msb first
    for i in (0..77).rev() {
        bits.push((message.bits() >> i) & 1 != 0);
    }

    // push th 14 bits of crc
    for i in (0..14).rev() {
        bits.push((message.checksum() >> i) & 1 != 0);
    }

    // push 83 bits of parity
    for i in (0..83).rev() {
        bits.push((parity >> i) & 1 != 0);
    }

    // convert the bits into 3 bit symbols
    let mut symbols:Vec<u8> = vec![];
    for chunk in bits.chunks_exact(3) {
        let value = chunk.load_be::<u8>() & 0b0000_0111;
        symbols.push(value);
    }
    
    // gray encode
    let gray = GrayCode::new(&FT8_GRAY_CODE);
    let gray_encoded_symbols = gray.encode(&symbols);

    // insert costas
    let costas:Vec<u8> = vec![3,1,4,0,6,5,2];
    let mut channel_symbols:Vec<u8> = vec![];
    channel_symbols.extend_from_slice(&costas);
    channel_symbols.extend_from_slice(&gray_encoded_symbols[0..29]);
    channel_symbols.extend_from_slice(&costas);
    channel_symbols.extend_from_slice(&gray_encoded_symbols[29..]);
    channel_symbols.extend_from_slice(&costas);

    print!("Channel symbols: ");
    for symbol in channel_symbols.iter() {
        print!("{}", symbol);
    }
    println!();

}
