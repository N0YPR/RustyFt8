//! FT8 Signal Simulator - Similar to WSJT-X ft8sim
//!
//! Generate FT8 signals with realistic propagation effects:
//! - Adjustable SNR (signal-to-noise ratio)
//! - Frequency offset (simulates Doppler shift)
//! - Time delay (simulates late arrival)
//! - Signal amplitude control
//!
//! Usage:
//!   cargo run --bin ft8sim -- [OPTIONS] <message> <output.wav>
//!   ft8sim [OPTIONS] <message> <output.wav>
//!
//! Options:
//!   -s, --snr <dB>        Signal-to-noise ratio in dB (default: 0)
//!   -f, --freq <Hz>       Base frequency in Hz (default: 1500)
//!   -d, --delay <sec>     Time delay in seconds (default: 0.0)
//!   -n, --noise           Add AWGN noise
//!   -h, --help            Show this help message
//!
//! Examples:
//!   # Clean signal at 1500 Hz
//!   ft8sim "CQ N0YPR DM42" output.wav
//!
//!   # Weak signal with noise (SNR = -10 dB)
//!   ft8sim -s -10 -n "CQ N0YPR DM42" output.wav
//!
//!   # Signal at 1000 Hz with 0.5s delay
//!   ft8sim -f 1000 -d 0.5 "CQ SOTA N0YPR DM42" output.wav

use rustyft8::{crc, encode, ldpc, pulse, symbol, wav};
use rustyft8::message::CallsignHashCache;
use bitvec::prelude::*;

struct SimConfig {
    message: String,
    output_path: String,
    snr_db: f32,
    base_freq: f32,
    time_delay: f32,
    add_noise: bool,
}

impl SimConfig {
    fn parse_args() -> Result<Self, String> {
        let args: Vec<String> = std::env::args().collect();

        let mut snr_db = 0.0;
        let mut base_freq = 1500.0;
        let mut time_delay = 0.0;
        let mut add_noise = false;
        let mut message = None;
        let mut output_path = None;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-s" | "--snr" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("Missing value for --snr".to_string());
                    }
                    snr_db = args[i].parse()
                        .map_err(|_| format!("Invalid SNR value: {}", args[i]))?;
                }
                "-f" | "--freq" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("Missing value for --freq".to_string());
                    }
                    base_freq = args[i].parse()
                        .map_err(|_| format!("Invalid frequency value: {}", args[i]))?;
                }
                "-d" | "--delay" => {
                    i += 1;
                    if i >= args.len() {
                        return Err("Missing value for --delay".to_string());
                    }
                    time_delay = args[i].parse()
                        .map_err(|_| format!("Invalid delay value: {}", args[i]))?;
                }
                "-n" | "--noise" => {
                    add_noise = true;
                }
                "-h" | "--help" => {
                    print_help(&args[0]);
                    std::process::exit(0);
                }
                arg if !arg.starts_with('-') => {
                    if message.is_none() {
                        message = Some(arg.to_string());
                    } else if output_path.is_none() {
                        output_path = Some(arg.to_string());
                    } else {
                        return Err(format!("Unexpected argument: {}", arg));
                    }
                }
                arg => return Err(format!("Unknown option: {}", arg)),
            }
            i += 1;
        }

        let message = message.ok_or("Missing message argument")?;
        let output_path = output_path.ok_or("Missing output file argument")?;

        Ok(SimConfig {
            message,
            output_path,
            snr_db,
            base_freq,
            time_delay,
            add_noise,
        })
    }
}

fn print_help(program: &str) {
    eprintln!("FT8 Signal Simulator");
    eprintln!();
    eprintln!("Usage: {} [OPTIONS] <message> <output.wav>", program);
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -s, --snr <dB>        Signal-to-noise ratio in dB (default: 0)");
    eprintln!("  -f, --freq <Hz>       Base frequency in Hz (default: 1500)");
    eprintln!("  -d, --delay <sec>     Time delay in seconds (default: 0.0)");
    eprintln!("  -n, --noise           Add AWGN noise");
    eprintln!("  -h, --help            Show this help message");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {} \"CQ N0YPR DM42\" output.wav", program);
    eprintln!("  {} -s -10 -n \"CQ N0YPR DM42\" weak_signal.wav", program);
    eprintln!("  {} -f 1000 -d 0.5 \"CQ SOTA N0YPR DM42\" delayed.wav", program);
}

/// Generate white Gaussian noise (exactly like WSJT-X gran())
///
/// Uses the Marsaglia polar method (variant of Box-Muller) to generate
/// Gaussian-distributed random numbers with mean=0 and std_dev=1.
/// This matches WSJT-X's gran() implementation.
fn generate_gaussian_noise(num_samples: usize) -> Vec<f32> {
    let mut noise = Vec::with_capacity(num_samples);
    let mut rng_state = 12345u32; // Simple LCG for deterministic noise
    let mut have_spare = false;
    let mut spare = 0.0f32;

    for _ in 0..num_samples {
        if have_spare {
            // Use the spare value from previous iteration
            noise.push(spare);
            have_spare = false;
        } else {
            // Marsaglia polar method (same as WSJT-X gran.c)
            let (v1, v2, rsq) = loop {
                // Generate two uniform random numbers in [-1, 1]
                rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                let v1 = 2.0 * (rng_state as f32 / u32::MAX as f32) - 1.0;

                rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
                let v2 = 2.0 * (rng_state as f32 / u32::MAX as f32) - 1.0;

                let rsq = v1 * v1 + v2 * v2;

                // Keep only points inside the unit circle
                if rsq < 1.0 && rsq > 0.0 {
                    break (v1, v2, rsq);
                }
            };

            let fac = ((-2.0 * rsq.ln()) / rsq).sqrt();
            spare = v1 * fac;
            have_spare = true;
            noise.push(v2 * fac);
        }
    }

    noise
}

/// Calculate signal and noise power, add noise to achieve target SNR (WSJT-X method)
///
/// This implements the exact SNR approach as WSJT-X ft8sim:
/// 1. Scale signal by: sig = sqrt(2 * bandwidth_ratio) * 10^(snr_db / 20)
/// 2. Add unit-variance Gaussian noise (mean=0, std_dev=1)
/// 3. The resulting waveform is in the range ~[-3, 3] for typical SNR values
///
/// bandwidth_ratio = 2500 Hz / (sample_rate / 2)
fn add_noise_for_snr(signal: &mut [f32], snr_db: f32, _center_freq: f32, sample_rate: f32) {
    // WSJT-X approach: SNR is defined in 2500 Hz bandwidth
    let bandwidth_ratio = 2500.0 / (sample_rate / 2.0);

    // Calculate signal scaling factor (WSJT-X formula)
    // sig = sqrt(2 * bandwidth_ratio) * 10^(0.05 * snrdb)
    let sig_scale = (2.0 * bandwidth_ratio).sqrt() * 10.0f32.powf(0.05 * snr_db);

    // First, scale the signal down
    for s in signal.iter_mut() {
        *s = sig_scale * (*s);
    }

    // Generate white Gaussian noise (mean=0, std_dev=1)
    let noise = generate_gaussian_noise(signal.len());

    // Calculate signal and noise power (for verification)
    let signal_power: f32 = signal.iter().map(|&s| s * s).sum::<f32>() / signal.len() as f32;
    let noise_power: f32 = noise.iter().map(|&n| n * n).sum::<f32>() / noise.len() as f32;

    // Adjusted noise power in 2500 Hz bandwidth
    let noise_power_2500 = bandwidth_ratio * noise_power;
    let actual_snr = 10.0 * (signal_power / noise_power_2500).log10();

    println!("  Signal power: {:.6}", signal_power);
    println!("  Noise power (2500 Hz BW): {:.6}", noise_power_2500);
    println!("  Target SNR: {:.1} dB, Actual: {:.1} dB", snr_db, actual_snr);

    // Add unit-variance Gaussian noise directly (WSJT-X method)
    // After this, the signal is in range approximately [-3, 3] for typical SNR
    for (s, n) in signal.iter_mut().zip(noise.iter()) {
        *s += n;
    }

    // Apply final gain to match WSJT-X output levels
    // WSJT-X output: RMS ~100/32767 = 0.003
    // Our waveform after noise: RMS ~1.5
    // After f32->i16 conversion (x32767): RMS ~49000
    // Need to scale by: 100/49000 = 0.002
    let gain = 0.003; // Brings output to WSJT-X levels
    for s in signal.iter_mut() {
        *s *= gain;
    }
}

/// Apply time delay by prepending silence
fn apply_time_delay(signal: &[f32], delay_sec: f32, sample_rate: f32) -> Vec<f32> {
    let delay_samples = (delay_sec * sample_rate) as usize;

    let mut delayed = vec![0.0f32; delay_samples + signal.len()];
    delayed[delay_samples..].copy_from_slice(signal);

    delayed
}

fn main() -> Result<(), String> {
    let config = SimConfig::parse_args()?;

    println!("FT8 Signal Simulator");
    println!("===================");
    println!("Message:      {}", config.message);
    println!("Base freq:    {:.1} Hz", config.base_freq);
    println!("SNR:          {:.1} dB", config.snr_db);
    println!("Time delay:   {:.3} s", config.time_delay);
    println!("Add noise:    {}", if config.add_noise { "yes" } else { "no" });
    println!();

    // Step 1: Encode message to 77 bits
    println!("Step 1: Encoding message...");
    let mut message_storage = [0u8; 10];
    let message_bits = &mut message_storage.view_bits_mut::<Msb0>()[..77];

    let mut hash_cache = CallsignHashCache::new();
    encode(&config.message, message_bits, &mut hash_cache)?;

    // Step 2: Add CRC-14
    let mut msg_with_crc_storage = [0u8; 12];
    let msg_with_crc = &mut msg_with_crc_storage.view_bits_mut::<Msb0>()[..91];
    msg_with_crc[0..77].copy_from_bitslice(&message_bits[0..77]);

    let crc_value = crc::crc14(&message_bits[0..77]);
    for i in 0..14 {
        msg_with_crc.set(77 + i, (crc_value & (1 << (13 - i))) != 0);
    }

    // Step 3: LDPC encode
    let mut codeword_storage = [0u8; 22];
    let codeword = &mut codeword_storage.view_bits_mut::<Msb0>()[..174];
    ldpc::encode(&msg_with_crc[0..91], codeword);

    // Step 4: Map to symbols
    let mut symbols = [0u8; 79];
    symbol::map(codeword, &mut symbols)?;
    println!("  ✓ Encoded to 79 symbols");

    // Step 5: Generate waveform
    println!("Step 2: Generating waveform...");
    let mut pulse_buf = vec![0.0f32; 3 * pulse::NSPS];
    pulse::compute_pulse(&mut pulse_buf, pulse::BT, pulse::NSPS)?;

    let num_samples = 79 * pulse::NSPS;
    let mut waveform = vec![0.0f32; num_samples];

    pulse::generate_waveform(
        &symbols,
        &mut waveform,
        &pulse_buf,
        config.base_freq,
        pulse::SAMPLE_RATE,
        pulse::NSPS
    )?;

    let duration = num_samples as f32 / pulse::SAMPLE_RATE;
    println!("  ✓ Generated {:.2}s waveform", duration);

    // Step 6: Apply propagation effects
    if config.add_noise {
        println!("Step 3: Adding noise...");
        add_noise_for_snr(&mut waveform, config.snr_db, config.base_freq, pulse::SAMPLE_RATE);
        println!("  ✓ Added white Gaussian noise (SNR in 2500 Hz BW)");
    }

    let mut final_waveform = if config.time_delay > 0.0 {
        println!("Step 4: Applying time delay...");
        let delayed = apply_time_delay(&waveform, config.time_delay, pulse::SAMPLE_RATE);
        let delay_ms = config.time_delay * 1000.0;
        println!("  ✓ Added {:.1} ms delay", delay_ms);
        delayed
    } else {
        waveform
    };

    // Step 7: Pad to 15 seconds (WSJT-X format)
    // WSJT-X uses NMAX = 15 * 12000 = 180,000 samples
    println!("Step 5: Padding to 15 seconds...");
    let target_samples = (15.0 * pulse::SAMPLE_RATE) as usize; // 180,000 samples
    if final_waveform.len() < target_samples {
        let original_len = final_waveform.len();
        let padding = target_samples - original_len;
        final_waveform.resize(target_samples, 0.0);

        // If noise was added, fill padding with noise at the same level
        if config.add_noise {
            let padding_noise = generate_gaussian_noise(padding);
            let gain = 0.003; // Same gain as main signal
            for (i, n) in padding_noise.iter().enumerate() {
                final_waveform[original_len + i] = n * gain;
            }
        }
        println!("  ✓ Padded to {:.1}s ({} samples)", 15.0, target_samples);
    }

    // Step 8: Write WAV file
    println!("Step 6: Writing WAV file...");
    wav::write_wav_file(&config.output_path, &final_waveform, pulse::SAMPLE_RATE as u32)?;

    let file_size_kb = (44 + final_waveform.len() * 2) as f32 / 1024.0;
    let total_duration = final_waveform.len() as f32 / pulse::SAMPLE_RATE;
    println!("  ✓ Written to: {}", config.output_path);
    println!("  ✓ File size: {:.1} KB", file_size_kb);
    println!("  ✓ Duration: {:.2} s", total_duration);

    println!();
    println!("✓ Simulation complete!");

    Ok(())
}
