use std::{env, vec};

use bitvec::prelude::*;
use constants::SAMPLE_RATE;
use encode::{gray::{GrayCode, FT8_GRAY_CODE}, ldpc::Ldpc};
use hound::{WavSpec, WavWriter};
use message::message::Message;
use modulation::Modulator;
use simulation::noise::*;

mod constants;
mod message;
mod encode;
mod modulation;
mod simulation;
mod util;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <message>", args[0]);
        std::process::exit(1);
    }
    let message_str: &str = &args[1];

    let delta_time = 0.0;

    let message = Message::try_from(message_str).unwrap();

    println!("Message: {}", message);

    println!("Message bits: {:077b}", message.bits());

    println!("Checksum: {:014b}", message.checksum());

    let message_plus_checksum = (message.bits() << 14) | message.checksum() as u128;
    println!("Message & Checksum: {:091b}", message_plus_checksum);

    let ldpc = Ldpc::new();
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

    let modulator = Modulator::new();
    let waveform = modulator.modulate(&channel_symbols, 1040.0);

    // Apply QSB to our signal (Slow Fading)
    //let waveform = apply_qsb(&waveform, SAMPLE_RATE as u32, QSB_FREQ_HZ);

    // Introduce weak carrier-like fluttering to our signal
    //let waveform = apply_fluttering(&waveform, SAMPLE_RATE as u32, FLUTTER_FREQ_HZ);

    // Calculate noise standard deviation and power
    let noise_db = 30.0;
    let noise_sigma = (10.0_f32).powf(noise_db / 20.0);
    let noise_power = 2500_f32 / SAMPLE_RATE * 2_f32 * noise_sigma * noise_sigma;  // Noise power in 2.5kHz

    // generate white noise
    let mut signal = generate_white_noise(15 * SAMPLE_RATE as usize, noise_sigma);

    // Calculate signal amplitude
    let snr = -10_f32;
    let tx_power = noise_power * 10_f32.powf(snr / 10_f32);
    let amplitude = (2.0_f32 * tx_power).sqrt();

    // add our waveform
    //let amplitude = 0.25;
    let starting_sample = ((0.5 + delta_time) * SAMPLE_RATE) as usize;
    for i in 0..signal.len() {
        if i < starting_sample {
            continue;
        }

        if i >= waveform.len() {
            break;
        }

        signal[i] = signal[i] + waveform[i] * amplitude;
    }

    // Apply Band-Pass Filter (300 Hz – 2700 Hz)
    let signal = apply_bandpass_filter(&signal, SAMPLE_RATE as u32, LOW_CUTOFF_HZ, HIGH_CUTOFF_HZ);

    
    let wavspec = WavSpec {
        channels: 1,
        sample_rate: 12000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int
    };
    let mut writer = WavWriter::create("output.wav", wavspec).unwrap();

    for &sample in &signal {
        let int_sample = (sample * i16::MAX as f32) as i16;
        writer.write_sample(int_sample).unwrap();
    }
}
