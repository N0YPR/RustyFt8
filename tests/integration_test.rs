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

/// Decode FT8 signals with custom configuration
/// Returns all decoded messages (including possible false positives)
fn decode_signals_with_config(signal: &[f32], max_count: usize, config: &DecoderConfig) -> Vec<String> {
    let mut messages = Vec::new();

    match decode_ft8(signal, config, |msg| {
        eprintln!("✓ Decoded: {:.1} Hz @ {:.3} s - \"{}\"", msg.frequency, msg.time_offset, msg.message);
        messages.push(msg.message);
        // Continue until we hit max_count (allows finding real signals after false positives)
        messages.len() < max_count
    }) {
        Ok(_) => messages,
        Err(_) => Vec::new(),
    }
}

/// Decode FT8 signals with default configuration
fn decode_signals(signal: &[f32], max_count: usize) -> Vec<String> {
    let config = DecoderConfig::default();
    decode_signals_with_config(signal, max_count, &config)
}

/// Test encode→decode round trip at a specific SNR
///
/// Tests complete FT8 pipeline from message encoding through decoding at specified SNR.
/// Uses multi-candidate decoding to handle spurious sync peaks.
/// Allows false positives - only checks that expected message is decoded.
fn test_roundtrip(message: &str, snr_db: f32, should_succeed: bool) {
    test_roundtrip_with_config(message, snr_db, should_succeed, &DecoderConfig::default());
}

/// Test encode→decode round trip with custom decoder config
fn test_roundtrip_with_config(message: &str, snr_db: f32, should_succeed: bool, config: &DecoderConfig) {
    eprintln!("\n=== Testing \"{}\" at SNR = {} dB ===", message, snr_db);

    // Generate signal
    let signal = generate_test_signal(message, snr_db, 1500.0, 0.0);

    // Verify signal properties
    assert_eq!(signal.len(), NMAX, "Signal length should be 15 seconds");
    let has_signal = signal.iter().any(|&x| x.abs() > 1e-6);
    assert!(has_signal, "Signal should contain non-zero samples");
    eprintln!("✓ Signal generated successfully");

    // Attempt decode using the multi-signal decoder
    // Allow decoding multiple candidates to handle false positives
    let decoded_messages = decode_signals_with_config(&signal, 10, config);

    // Check if expected message is in the decoded list (false positives are OK)
    let found = decoded_messages.iter().any(|m| m == message);

    if found {
        eprintln!("✓ Decoded expected message: \"{}\"", message);
        if decoded_messages.len() > 1 {
            eprintln!("  (Also decoded {} other signal(s) - false positives)",
                decoded_messages.len() - 1);
        }
    } else {
        if should_succeed {
            panic!("Expected message \"{}\" not found at {} dB SNR. Decoded: {:?}",
                message, snr_db, decoded_messages);
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
#[ignore] // Slow test (~15s) - run with: cargo test -- --ignored
fn test_roundtrip_perfect_signal() {
    test_roundtrip("CQ W1ABC FN42", f32::INFINITY, true);
}

#[test]
#[ignore] // Slow test (~15s) - run with: cargo test -- --ignored
fn test_roundtrip_good_snr() {
    test_roundtrip("CQ W1ABC FN42", 10.0, true);
}

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_roundtrip_moderate_snr() {
    test_roundtrip("CQ W1ABC FN42", -10.0, true);
}

#[test]
#[cfg_attr(debug_assertions, ignore)] // Fast in release mode (0.6s), slow in debug (40s)
fn test_roundtrip_near_threshold() {
    // Test near threshold (-15 dB)
    // Run with: cargo test --release (for fast execution)
    let fast_config = DecoderConfig {
        freq_min: 1000.0,     // Narrow range around test signal (1500 Hz)
        freq_max: 2000.0,
        sync_threshold: 0.5,
        max_candidates: 20,
        decode_top_n: 3,      // Minimal decoding for speed
        min_snr_db: -18,
        enable_ap: false,
        mycall: None,
        hiscall: None,
    };
    test_roundtrip_with_config("CQ W1ABC FN42", -15.0, true, &fast_config);
}

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_roundtrip_below_threshold() {
    // Test well below threshold (-19 dB) - should fail
    test_roundtrip("CQ W1ABC FN42", -19.0, false);
}

#[test]
#[ignore] // Slow test - run with: cargo test -- --ignored
fn test_roundtrip_different_messages() {
    // Test various message types at good SNR for speed
    test_roundtrip("CQ DX K1ABC FN42", 0.0, true);
    test_roundtrip("K1ABC W9XYZ RRR", 0.0, true);
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
#[ignore] // Slow test - run with: cargo test -- --ignored
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

    // Decode using a FAST config for testing
    let config = DecoderConfig {
        freq_min: 100.0,
        freq_max: 3000.0,
        sync_threshold: 0.5,
        max_candidates: 50,
        decode_top_n: 10,  // Need a few more for multi-signal
        min_snr_db: -20,
        enable_ap: false,
        mycall: None,
        hiscall: None,
    };
    let mut decoded_messages = Vec::new();

    let count = decode_ft8(&mixed_signal, &config, |msg| {
        eprintln!("Decoded: {:.1} Hz - \"{}\"", msg.frequency, msg.message);
        decoded_messages.push((msg.frequency, msg.message.clone()));
        // Continue until we've decoded enough candidates (allow for false positives)
        decoded_messages.len() < 10
    }).expect("Decode failed");

    eprintln!();
    eprintln!("Total decoded: {}", count);

    // Check that we got both expected messages (false positives are allowed)
    let messages: Vec<String> = decoded_messages.iter().map(|(_, m)| m.clone()).collect();
    assert!(messages.contains(&"CQ W1ABC FN42".to_string()),
        "Missing expected signal: 'CQ W1ABC FN42'. Decoded: {:?}", messages);
    assert!(messages.contains(&"K1ABC W9XYZ RR73".to_string()),
        "Missing expected signal: 'K1ABC W9XYZ RR73'. Decoded: {:?}", messages);

    eprintln!("✓ Successfully decoded both expected signals (found {} total decodes)", count);
}
