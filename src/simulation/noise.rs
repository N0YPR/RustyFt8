use std::{f32::consts::PI, i16};

use rand::prelude::*;
use rand_distr::{Distribution, Normal, Uniform};

use biquad::*;

pub const LOW_CUTOFF_HZ: f32 = 300.0; // SSB low-end
pub const HIGH_CUTOFF_HZ: f32 = 2700.0; // SSB high-end
pub const QSB_FREQ_HZ: f32 = 0.2; // Slow fading rate (0.2 Hz = ~5-second cycles)
pub const FLUTTER_FREQ_HZ: f32 = 20.0; 

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
        pink_noise[i] = white[0] + white[1] + white[2] + white[3] + white[4] + white[5] + white[6] + white_noise * 0.5362;
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

    samples.iter().zip(qsb_signal.iter()).map(|(&s, &q)| s * q).collect()
}

// Introduce weak carrier-like fluttering (20 Hz amplitude wobble)
pub fn apply_fluttering(samples: &[f32], sample_rate: u32, flutter_freq: f32) -> Vec<f32> {
    let num_samples = samples.len();
    let mut flutter_signal = vec![0.0; num_samples];

    for i in 0..num_samples {
        let phase = 2.0 * PI * flutter_freq * (i as f32 / sample_rate as f32);
        flutter_signal[i] = 0.9 + 0.1 * phase.sin(); // Small amplitude modulation
    }

    samples.iter().zip(flutter_signal.iter()).map(|(&s, &f)| s * f).collect()
}

// Introduce occasional crackle by injecting random high-energy spikes
pub fn add_random_spikes(samples: &mut [f32], spike_probability: f32) {
    let mut rng = rand::rng();
    let spike_dist = Uniform::new(-1.5, 1.5).unwrap();

    for sample in samples.iter_mut() {
        if rng.gen::<f32>() < spike_probability {
            *sample += spike_dist.sample(&mut rng);
        }
    }
}

// Function to apply a band-pass filter using `biquad`
pub fn apply_bandpass_filter(samples: &[f32], sample_rate: u32, low_cutoff: f32, high_cutoff: f32) -> Vec<f32> {
    let low_pass = biquad::Coefficients::<f32>::from_params(
        Type::LowPass,
        sample_rate.hz(),
        high_cutoff.hz(),
        Q_BUTTERWORTH_F32,
    ).unwrap();

    let high_pass = biquad::Coefficients::<f32>::from_params(
        Type::HighPass,
        sample_rate.hz(),
        low_cutoff.hz(),
        Q_BUTTERWORTH_F32,
    ).unwrap();

    let mut low_filter = DirectForm2Transposed::<f32>::new(low_pass);
    let mut high_filter = DirectForm2Transposed::<f32>::new(high_pass);

    samples.iter().map(|&x| high_filter.run(low_filter.run(x))).collect()
}