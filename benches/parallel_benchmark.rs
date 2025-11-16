//! Benchmark to measure parallelization speedup
//!
//! Compares decode performance with full parallelism enabled

use rustyft8::{crc, encode, ldpc, pulse, symbol, decode_ft8, DecoderConfig};
use rustyft8::message::CallsignHashCache;
use bitvec::prelude::*;
use std::time::Instant;

const SAMPLE_RATE: f32 = 12000.0;
const NMAX: usize = 15 * 12000; // 15 seconds
const NSPS: usize = 1920; // Samples per symbol

/// Generate a complete FT8 waveform at specified frequency (no noise)
fn generate_ft8_signal_clean(message: &str, frequency: f32) -> Vec<f32> {
    // Encode message to 77 bits
    let mut cache = CallsignHashCache::new();
    let mut message_storage = [0u8; 10];
    let message_bits = &mut message_storage.view_bits_mut::<Msb0>()[..77];
    encode(message, message_bits, &mut cache).expect("Failed to encode");

    // Add CRC-14
    let mut msg_with_crc_storage = [0u8; 12];
    let msg_with_crc = &mut msg_with_crc_storage.view_bits_mut::<Msb0>()[..91];
    msg_with_crc[0..77].copy_from_bitslice(&message_bits[0..77]);

    let crc_value = crc::crc14(&message_bits[0..77]);
    for i in 0..14 {
        msg_with_crc.set(77 + i, (crc_value & (1 << (13 - i))) != 0);
    }

    // LDPC encode (91 bits → 174 bits)
    let mut codeword_storage = [0u8; 22];
    let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
    ldpc::encode(&msg_with_crc[0..91], codeword);

    // Map to FT8 symbols
    let mut symbols = [0u8; 79];
    symbol::map(codeword, &mut symbols).expect("Failed to map symbols");

    // Generate waveform
    let mut pulse_buf = vec![0.0f32; 3 * NSPS];
    pulse::compute_pulse(&mut pulse_buf, pulse::BT, NSPS).expect("Failed to compute pulse");

    // Waveform must be exactly 79 symbols * 1920 samples/symbol = 151680 samples
    let waveform_len = 79 * NSPS;
    let mut waveform = vec![0.0f32; waveform_len];
    pulse::generate_waveform(&symbols, &mut waveform, &pulse_buf[..], frequency, SAMPLE_RATE, NSPS)
        .expect("Failed to generate waveform");

    // Pad to 15 seconds (180000 samples) with zeros
    waveform.resize(NMAX, 0.0f32);

    waveform
}

/// Add AWGN noise to achieve target SNR
fn add_noise(signal: &mut [f32], snr_db: f32, seed: u32) {
    let signal_power: f32 = signal.iter().map(|x| x * x).sum::<f32>() / signal.len() as f32;
    let noise_power = signal_power / 10f32.powf(snr_db / 10.0);
    let noise_std = noise_power.sqrt();

    // Add white noise using simple LCG
    let mut rng_state = seed;
    for sample in signal.iter_mut() {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        let noise = (rng_state as f32 / u32::MAX as f32 - 0.5) * noise_std * 3.464; // uniform to ~gaussian
        *sample += noise;
    }
}

fn main() {
    println!("\n=== FT8 Parallel Decode Benchmark ===\n");

    // Test configurations: (name, num_signals, snr_db)
    let test_configs = vec![
        ("Light load", 2, 0.0),
        ("Medium load", 5, -5.0),
        ("Heavy load", 10, -10.0),
        ("Contest load (low SNR)", 8, -15.0),
        ("High density", 20, -8.0),
        ("Maximum density", 50, -5.0),
    ];

    for (name, num_signals, snr_db) in test_configs {
        println!("Test: {}", name);
        println!("  {} signals, {} dB SNR", num_signals, snr_db);

        // Generate multiple signals at different frequencies
        let mut mixed_signal = vec![0.0f32; NMAX];
        let messages = vec![
            "CQ W1ABC FN42", "K1JT W9XYZ RR73", "CQ DX N0YPR DM42", "W1ABC K1JT R-15",
            "CQ VE3ABC FN03", "G4ABC DL1ABC 73", "JA1XYZ K9ABC +05", "VK3ABC W8ABC RRR",
            "ZL1ABC PA3XYZ -12", "ON4ABC W2ABC R+03", "CQ UA1ABC KO48", "K2ABC N4XYZ R-08",
            "CQ EA3ABC JN11", "VE7ABC KL7XYZ +02", "W5ABC K8XYZ RR73", "CQ PY2ABC GG66",
            "ZS1ABC VK9XYZ -18", "LU3ABC W3XYZ +12", "CQ JA3ABC PM95", "VE2ABC K7XYZ RRR",
            "CQ ZL1ABC RF70", "OH1ABC K6XYZ +08", "SM5ABC W4XYZ R-22", "CQ PA3ABC JO21",
            "LA2ABC N8XYZ RR73", "G3ABC W7XYZ -05", "CQ DL3ABC JO62", "I2ABC VE1XYZ +15",
            "CQ F5ABC JN23", "SP1ABC K5XYZ R-11", "OK2ABC W6XYZ 73", "CQ YO5ABC KN16",
            "HA5ABC N3XYZ RRR", "S5ABC K4XYZ -14", "CQ 9A1ABC JN75", "LZ2ABC W2XYZ +03",
            "CQ UR5ABC KO50", "ES1ABC VE3XYZ R-19", "CT1ABC K3XYZ RR73", "CQ EA8ABC IL18",
            "GM3ABC N5XYZ -07", "EI7ABC W9XYZ +11", "CQ GW4ABC IO81", "ON5ABC K2XYZ R-16",
            "CQ OZ1ABC JO55", "LA3ABC N7XYZ 73", "SM7ABC W8XYZ RRR", "CQ OH2ABC KP20",
            "ES5ABC VE2XYZ -09", "LY1ABC K9XYZ +06",
        ];

        // Generate and mix clean signals
        // Adaptive frequency spacing: 50 Hz for high density, 150 Hz for low density
        let freq_spacing = if num_signals > 20 { 50.0 } else { 150.0 };
        let freq_start = 600.0;

        for i in 0..num_signals {
            let freq = freq_start + (i as f32) * freq_spacing;
            let signal = generate_ft8_signal_clean(messages[i % messages.len()], freq);
            for j in 0..NMAX {
                mixed_signal[j] += signal[j];
            }
        }

        // Add noise after mixing for accurate SNR
        add_noise(&mut mixed_signal, snr_db, 42);

        // Run decode benchmark (decode top 50 candidates)
        let freq_max = freq_start + (num_signals as f32) * freq_spacing + 200.0;
        let config = DecoderConfig {
            freq_min: 500.0,
            freq_max,
            decode_top_n: 50,
            ..DecoderConfig::default()
        };

        let start = Instant::now();
        let count = decode_ft8(&mixed_signal, &config, |_msg| true).expect("Decode failed");
        let elapsed = start.elapsed();

        println!("  Decoded {} messages in {:.2?}", count, elapsed);
        println!("  Throughput: {:.1} decodes/sec", count as f64 / elapsed.as_secs_f64());
        println!();
    }

    println!("=== CPU Information ===");
    println!("Rayon thread pool size: {}", rayon::current_num_threads());
}
