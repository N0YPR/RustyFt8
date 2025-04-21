#![allow(unused)]

use std::{f32::consts::PI, i16};

use rand::prelude::*;
use rand_distr::{Distribution, Normal, Uniform};

use biquad::*;

use crate::constants::SAMPLE_RATE;

pub const LOW_CUTOFF_HZ: f32 = 300.0; // SSB low-end
pub const HIGH_CUTOFF_HZ: f32 = 2700.0; // SSB high-end
pub const QSB_FREQ_HZ: f32 = 0.2; // Slow fading rate (0.2 Hz = ~5-second cycles)
pub const FLUTTER_FREQ_HZ: f32 = 20.0;
const SSB_BANDWIDTH: f32 = 2500.0; // Typical SSB bandwidth (Hz)
const S9_DBM: f32 = -73.0; // S9 level in dBm (50Ω system)
const IMPEDANCE: f32 = 50.0; // 50Ω impedance

/// Convert dBm to linear RMS voltage
pub fn dbm_to_voltage_rms(dbm: f32) -> f32 {
    let power_watts = 10.0_f32.powf(dbm / 10.0) / 1000.0; // Convert dBm to watts
    (power_watts * IMPEDANCE).sqrt() // Convert to RMS voltage
}

/// Compute RMS (Root Mean Square) Power of a signal
pub fn rms_power(signal: &[f32]) -> f32 {
    let sum_squares: f32 = signal.iter().map(|&x| x * x).sum();
    (sum_squares / signal.len() as f32).sqrt()
}

pub fn mix_waveform(
    samples: &mut Vec<f32>,
    noise_rms: f32,
    waveform: &Vec<f32>,
    start_index: usize,
    snr_db: f32,
) {
    assert!(
        waveform.len() <= samples.len(),
        "Waveform must not be longer than samples"
    );

    // Convert FT8-style SNR (dB) to linear scale
    let snr_linear = 10.0_f32.powf(snr_db / 10.0); // FT8 uses 10 instead of 20

    // Compute desired signal RMS power based on FT8 noise power estimate
    let desired_signal_rms = (noise_rms / (2500.0 / SAMPLE_RATE as f32)) * snr_linear;

    // Compute current signal RMS power
    let signal_rms = rms_power(&waveform);

    // Scale the signal to match desired power level
    let scaling_factor = desired_signal_rms / signal_rms;
    println!("{}", scaling_factor);

    for (i, &wave_sample) in waveform.iter().enumerate() {
        let target_index = start_index + i;
        if target_index < samples.len() {
            samples[target_index] += wave_sample * scaling_factor;
        } else {
            break; // Stop if waveform exceeds the samples length
        }
    }
}

// pub fn generate_white_noise_s9(num_samples: usize) -> Vec<f32> {
//     // Create a normal distribution with mean 0 and standard deviation 1
//     let normal = Normal::new(0.0, 1.0).unwrap();
//     let mut rng = rand::rng();

//     // Create a vector to hold the white noise samples
//     let mut noise_samples = Vec::with_capacity(num_samples);

//     // Calculate the scaling factor based on the desired S9 power (40 dBm)
//     let scaling_factor = 10.0_f32.powf(S9_DBM / 20.0); // 40 dBm -> linear scale
//     println!("generate_white_noise_s9 scaling_factor: {}", scaling_factor);

//     // Generate the white noise samples and scale them
//     for _ in 0..num_samples {
//         let noise = normal.sample(&mut rng);
//         noise_samples.push(noise * scaling_factor); // Scale the noise to the desired amplitude
//     }

//     noise_samples
// }

/// Generate Gaussian white noise at S9 level
pub fn generate_white_noise_s9(samples: usize) -> Vec<f32> {
    let normal = Normal::new(0.0, 1.0).unwrap();
    let s3_gain = 0.125; // Adjust gain for S3 level
    let mut rng = rand::rng();

    (0..samples)
        .map(|_| normal.sample(&mut rng) * s3_gain)
        .collect()
}

pub fn generate_white_noise(num_samples: usize, sigma: f32) -> Vec<f32> {
    let mut rng = rand::rng();
    let normal = Normal::new(0.0, sigma).unwrap();
    let mut white_noise = vec![0.0; num_samples];
    for i in 0..num_samples {
        white_noise[i] = normal.sample(&mut rng);
    }
    white_noise
}

// Generate Pink Noise (using a simple filter-based method)
pub fn generate_pink_noise(num_samples: u32, sigma: f32) -> Vec<f32> {
    let mut rng = rand::rng();
    let normal = Normal::new(0.0, sigma).unwrap();
    let mut pink_noise = vec![0.0; num_samples as usize];

    let mut white = [0.0; 7]; // 7 state variables for a pink noise filter

    for i in 0..num_samples as usize {
        let white_noise = normal.sample(&mut rng) as f32;

        // Apply Paul Kellet’s Pink Noise Filter Approximation
        white[0] = 0.99886 * white[0] + white_noise * 0.0555179;
        white[1] = 0.99332 * white[1] + white_noise * 0.0750759;
        white[2] = 0.96900 * white[2] + white_noise * 0.1538520;
        white[3] = 0.86650 * white[3] + white_noise * 0.3104856;
        white[4] = 0.55000 * white[4] + white_noise * 0.5329522;
        white[5] = -0.7616 * white[5] - white_noise * 0.0168980;
        pink_noise[i] = white[0]
            + white[1]
            + white[2]
            + white[3]
            + white[4]
            + white[5]
            + white[6]
            + white_noise * 0.5362;
        white[6] = white_noise * 0.115926;
    }

    pink_noise
}

// Apply QSB (Slow Amplitude Fading)
pub fn apply_qsb(samples: &[f32], sample_rate: u32, qsb_freq: f32) -> Vec<f32> {
    let num_samples = samples.len();
    let mut qsb_signal = vec![0.0; num_samples];

    for i in 0..num_samples {
        let phase = 2.0 * std::f32::consts::PI * qsb_freq * (i as f32 / sample_rate as f32);
        qsb_signal[i] = 0.5 * (1.0 + phase.sin()); // Slow AM fading (0.5 to 1.0)
    }

    samples
        .iter()
        .zip(qsb_signal.iter())
        .map(|(&s, &q)| s * q)
        .collect()
}

// Introduce weak carrier-like fluttering (20 Hz amplitude wobble)
pub fn apply_fluttering(samples: &[f32], sample_rate: u32, flutter_freq: f32) -> Vec<f32> {
    let num_samples = samples.len();
    let mut flutter_signal = vec![0.0; num_samples];

    for i in 0..num_samples {
        let phase = 2.0 * PI * flutter_freq * (i as f32 / sample_rate as f32);
        flutter_signal[i] = 0.9 + 0.1 * phase.sin(); // Small amplitude modulation
    }

    samples
        .iter()
        .zip(flutter_signal.iter())
        .map(|(&s, &f)| s * f)
        .collect()
}

// Introduce occasional crackle by injecting random high-energy spikes
pub fn add_random_spikes(samples: &mut [f32], spike_probability: f32) {
    let mut rng = rand::rng();
    let spike_dist = Uniform::new(-1.5, 1.5).unwrap();

    for sample in samples.iter_mut() {
        if rng.random::<f32>() < spike_probability {
            *sample += spike_dist.sample(&mut rng);
        }
    }
}

// Function to apply a band-pass filter using `biquad`
pub fn apply_bandpass_filter(
    samples: &[f32],
    sample_rate: u32,
    low_cutoff: f32,
    high_cutoff: f32,
) -> Vec<f32> {
    let low_pass = biquad::Coefficients::<f32>::from_params(
        Type::LowPass,
        sample_rate.hz(),
        high_cutoff.hz(),
        Q_BUTTERWORTH_F32,
    )
    .unwrap();

    let high_pass = biquad::Coefficients::<f32>::from_params(
        Type::HighPass,
        sample_rate.hz(),
        low_cutoff.hz(),
        Q_BUTTERWORTH_F32,
    )
    .unwrap();

    let mut low_filter = DirectForm2Transposed::<f32>::new(low_pass);
    let mut high_filter = DirectForm2Transposed::<f32>::new(high_pass);

    samples
        .iter()
        .map(|&x| high_filter.run(low_filter.run(x)))
        .collect()
}

pub fn normalize_signal(signal: &mut [f32]) {
    // Find the min and max values in the signal
    let (min_value, max_value) = signal
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &x| {
            (min.min(x), max.max(x))
        });

    // Calculate the scale factor to normalize the signal
    let scale_factor = if max_value == min_value {
        1.0 // Avoid division by zero, return signal as is if all values are the same
    } else {
        2.0 / (max_value - min_value)
    };

    let offset = -(max_value + min_value) / (max_value - min_value);

    // Normalize each sample
    for sample in signal.iter_mut() {
        *sample = scale_factor * (*sample) + offset;
    }
}
