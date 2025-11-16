//! Integration tests for FT8 encode→decode round trips
//!
//! Tests the complete pipeline at various SNR levels to verify end-to-end functionality

use rustyft8::{crc, encode, ldpc, pulse, symbol, decode_ft8, DecoderConfig};
use rustyft8::message::CallsignHashCache;
use bitvec::prelude::*;
use std::f32;

const SAMPLE_RATE: f32 = 12000.0;
const NMAX: usize = 15 * 12000; // 15 seconds
const NSPS: usize = 1920; // Samples per symbol

/// Generate white Gaussian noise using Marsaglia polar method
fn generate_gaussian_noise(num_samples: usize, seed: u32) -> Vec<f32> {
    let mut noise = Vec::with_capacity(num_samples);
    let mut rng_state = seed;
    let mut have_spare = false;
    let mut spare = 0.0f32;

    for _ in 0..num_samples {
        if have_spare {
            noise.push(spare);
            have_spare = false;
        } else {
            let mut u: f32;
            let mut v: f32;
            let mut s: f32;

            loop {
                // Generate uniform random in [-1, 1]
                rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                u = (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0;

                rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                v = (rng_state as f32 / u32::MAX as f32) * 2.0 - 1.0;

                s = u * u + v * v;
                if s < 1.0 && s > 0.0 {
                    break;
                }
            }

            let scale = (-2.0 * s.ln() / s).sqrt();
            noise.push(u * scale);
            spare = v * scale;
            have_spare = true;
        }
    }

    noise
}

/// Generate FT8 signal with specified SNR
///
/// SNR is defined in 2500 Hz bandwidth as per FT8 standard
fn generate_test_signal(message: &str, snr_db: f32, freq_hz: f32, time_delay: f32) -> Vec<f32> {
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

    let num_samples = 79 * NSPS;
    let mut waveform = vec![0.0f32; num_samples];
    pulse::generate_waveform(
        &symbols,
        &mut waveform,
        &pulse_buf,
        freq_hz,
        SAMPLE_RATE,
        NSPS
    ).expect("Failed to generate waveform");

    // Add noise to waveform BEFORE placing in buffer (match ft8sim's WSJT-X approach)
    if snr_db.is_finite() && snr_db < 100.0 {
        // WSJT-X approach: SNR is defined in 2500 Hz bandwidth
        let bandwidth_ratio = 2500.0 / (SAMPLE_RATE / 2.0);

        // Calculate signal scaling factor (WSJT-X formula)
        // sig = sqrt(2 * bandwidth_ratio) * 10^(0.05 * snrdb)
        let sig_scale = (2.0 * bandwidth_ratio).sqrt() * 10.0f32.powf(0.05 * snr_db);

        // Scale the signal
        for s in waveform.iter_mut() {
            *s = sig_scale * (*s);
        }

        // Generate unit-variance Gaussian noise
        let noise = generate_gaussian_noise(waveform.len(), 12345);

        // Add unit-variance noise directly (WSJT-X method)
        for i in 0..waveform.len() {
            waveform[i] += noise[i];
        }

        // Apply final gain to match WSJT-X output levels
        let gain = 0.003;
        for s in waveform.iter_mut() {
            *s *= gain;
        }
    }

    // Apply time delay by prepending silence (like ft8sim does)
    let delay_samples = (time_delay * SAMPLE_RATE) as usize;
    let mut signal = vec![0.0f32; delay_samples + waveform.len()];
    signal[delay_samples..].copy_from_slice(&waveform);

    // Pad to 15 seconds (NMAX samples)
    if signal.len() < NMAX {
        signal.resize(NMAX, 0.0);
    }

    signal
}

/// Decode all FT8 signals in the recording using the multi-signal decoder
/// Returns all decoded messages
fn decode_all_signals(signal: &[f32]) -> Vec<String> {
    // Use optimized config for fast testing (decode top 5 candidates only)
    // This is sufficient for single-signal tests and much faster
    let config = DecoderConfig {
        decode_top_n: 5,
        ..DecoderConfig::default()
    };
    let mut messages = Vec::new();

    match decode_ft8(signal, &config, |msg| {
        eprintln!("✓ Decoded: {:.1} Hz @ {:.3} s - \"{}\"", msg.frequency, msg.time_offset, msg.message);
        messages.push(msg.message);
    }) {
        Ok(_) => messages,
        Err(_) => Vec::new(),
    }
}

/// Test encode→decode round trip at a specific SNR
///
/// Tests complete FT8 pipeline from message encoding through decoding at specified SNR.
/// Uses multi-candidate decoding to handle spurious sync peaks.
fn test_roundtrip(message: &str, snr_db: f32, should_succeed: bool) {
    eprintln!("\n=== Testing \"{}\" at SNR = {} dB ===", message, snr_db);

    // Generate signal
    let signal = generate_test_signal(message, snr_db, 1500.0, 0.0);

    // Verify signal properties
    assert_eq!(signal.len(), NMAX, "Signal length should be 15 seconds");
    let has_signal = signal.iter().any(|&x| x.abs() > 1e-6);
    assert!(has_signal, "Signal should contain non-zero samples");
    eprintln!("✓ Signal generated successfully");

    // Attempt decode using the multi-signal decoder
    let decoded_messages = decode_all_signals(&signal);

    if !decoded_messages.is_empty() {
        // For single-signal tests, we expect exactly one message
        assert_eq!(decoded_messages.len(), 1, "Expected exactly one decoded message");
        let decoded = &decoded_messages[0];
        assert_eq!(decoded, message, "Decoded message doesn't match expected");
    } else {
        if should_succeed {
            panic!("Decode failed unexpectedly at {} dB SNR", snr_db);
        } else {
            eprintln!("✓ Expected failure at {} dB SNR", snr_db);
        }
    }
}

#[test]
fn test_ldpc_constants() {
    // Sanity check that LDPC constants are accessible
    use rustyft8::ldpc;
    eprintln!("Testing LDPC encoder directly...");

    // Create a known-good 91-bit message (all zeros)
    let msg = bitvec::vec::BitVec::<u8, bitvec::order::Msb0>::repeat(false, 91);
    let mut codeword = bitvec::vec::BitVec::<u8, bitvec::order::Msb0>::repeat(false, 174);

    // This should not panic
    ldpc::encode(&msg, &mut codeword);

    eprintln!("✓ LDPC encode works");

    // Try to decode the all-zeros codeword
    let llr = vec![5.0f32; 174]; // Strong confidence all bits are 1 (wrong)
    let result = ldpc::decode(&llr, 100);
    eprintln!("LDPC decode result: {}", if result.is_some() { "Some" } else { "None" });

    // Try with correct LLRs (negative = bit is 0)
    let llr_correct = vec![-5.0f32; 174];
    let result2 = ldpc::decode(&llr_correct, 100);
    eprintln!("LDPC decode with correct LLRs: {}", if result2.is_some() { "Some" } else { "None" });
}

#[test]
fn test_roundtrip_perfect_signal() {
    test_roundtrip("CQ W1ABC FN42", f32::INFINITY, true);
}

#[test]
fn test_roundtrip_plus_10db() {
    test_roundtrip("CQ W1ABC FN42", 10.0, true);
}

#[test]
fn test_roundtrip_0db() {
    test_roundtrip("CQ W1ABC FN42", 0.0, true);
}

#[test]
fn test_roundtrip_minus_10db() {
    test_roundtrip("CQ W1ABC FN42", -10.0, true);
}

#[test]
fn test_roundtrip_minus_14db() {
    test_roundtrip("CQ W1ABC FN42", -14.0, true);
}

#[test]
fn test_roundtrip_minus_15db() {
    test_roundtrip("CQ W1ABC FN42", -15.0, true);
}

#[test]
fn test_roundtrip_minus_16db() {
    test_roundtrip("CQ W1ABC FN42", -16.0, true);
}

#[test]
fn test_roundtrip_minus_17db() {
    test_roundtrip("CQ W1ABC FN42", -17.0, true);
}

#[test]
fn test_roundtrip_minus_18db() {
    // This is our minimum working SNR
    test_roundtrip("CQ W1ABC FN42", -18.0, true);
}

#[test]
fn test_roundtrip_minus_19db() {
    // This should fail - below our minimum SNR
    test_roundtrip("CQ W1ABC FN42", -19.0, false);
}

#[test]
fn test_roundtrip_different_messages() {
    // Test various message types
    test_roundtrip("CQ DX K1ABC FN42", -10.0, true);
    test_roundtrip("K1ABC W9XYZ R-15", -10.0, true);
    test_roundtrip("W9XYZ K1ABC RRR", -10.0, true);
    test_roundtrip("K1ABC W9XYZ 73", -10.0, true);
}

#[test]
#[ignore] // Slow test - run with: cargo test --test integration_test -- --ignored
fn test_roundtrip_comprehensive_snr_sweep() {
    // Test every dB from -20 to +10
    for snr_db in -20..=10 {
        let should_succeed = snr_db >= -18;
        eprintln!("\n--- SNR = {} dB (expect: {}) ---",
                  snr_db, if should_succeed { "PASS" } else { "FAIL" });
        test_roundtrip("CQ W1ABC FN42", snr_db as f32, should_succeed);
    }
}

#[test]
fn test_multi_signal_decode() {
    eprintln!("\n=== Testing Multiple Simultaneous Signals ===");

    // Generate two signals at different frequencies and time offsets
    let signal1 = generate_test_signal("CQ W1ABC FN42", 0.0, 1000.0, 0.0);
    let signal2 = generate_test_signal("K1ABC W9XYZ RR73", 0.0, 2000.0, 0.0);

    // Mix the signals together (like real-world)
    let mut mixed_signal = vec![0.0f32; NMAX];
    for i in 0..NMAX {
        mixed_signal[i] = signal1[i] + signal2[i];
    }

    eprintln!("Generated 2 signals:");
    eprintln!("  Signal 1: 1000 Hz - \"CQ W1ABC FN42\"");
    eprintln!("  Signal 2: 2000 Hz - \"K1ABC W9XYZ RR73\"");
    eprintln!();

    // Decode using the multi-signal decoder
    let config = DecoderConfig {
        decode_top_n: 10, // Only decode top 10 candidates for faster testing
        ..DecoderConfig::default()
    };
    let mut decoded_messages = Vec::new();

    let count = decode_ft8(&mixed_signal, &config, |msg| {
        eprintln!("Decoded: {:.1} Hz - \"{}\"", msg.frequency, msg.message);
        decoded_messages.push((msg.frequency, msg.message.clone()));
    }).expect("Decode failed");

    eprintln!();
    eprintln!("Total decoded: {}", count);

    // Verify both signals were decoded
    assert_eq!(count, 2, "Expected to decode 2 signals");

    // Check that we got both messages
    let messages: Vec<String> = decoded_messages.iter().map(|(_, m)| m.clone()).collect();
    assert!(messages.contains(&"CQ W1ABC FN42".to_string()), "Missing first message");
    assert!(messages.contains(&"K1ABC W9XYZ RR73".to_string()), "Missing second message");

    // Verify approximate frequencies
    for (freq, _) in &decoded_messages {
        assert!(*freq > 900.0 && *freq < 1100.0 || *freq > 1900.0 && *freq < 2100.0,
                "Frequency {} out of expected range", freq);
    }

    eprintln!("✓ Successfully decoded both signals");
}
