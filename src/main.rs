use std::arch::x86_64;
use std::env;

use constants::{SAMPLE_RATE, SYMBOL_RATE};
use error_correction::ldpc::Ft8_Ldpc;
use hound::{WavSpec, WavWriter};
use message::Message;
use modulation::Modulator;
use plotters::prelude::*;
use rustfft::{num_complex::Complex, FftPlanner};
use simulation::noise::{apply_bandpass_filter, generate_white_noise_s9, mix_waveform, rms_power, HIGH_CUTOFF_HZ, LOW_CUTOFF_HZ};
use sonogram::{ColourGradient, FrequencyScale, SpecOptionsBuilder, Spectrogram};
use std::time::Instant;
use plotly::common::{ColorScalePalette, Mode};
use plotly::{HeatMap, Plot, Scatter};

mod constants;
mod error_correction;
mod message;
mod modulation;
mod simulation;
mod util;

// fn mix_waveform(
//     samples: &mut Vec<f32>, 
//     waveform: &Vec<f32>, 
//     start_index: usize, 
//     amplitude: f32
// ) {
//     for (i, &wave_sample) in waveform.iter().enumerate() {
//         let target_index = start_index + i;
//         if target_index < samples.len() {
//             samples[target_index] += wave_sample * amplitude;
//         } else {
//             break; // Stop if waveform exceeds the samples length
//         }
//     }
// }

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <message>", args[0]);
        std::process::exit(1);
    }
    let message_str: &str = &args[1];

    let delta_time = 0.0;
    let carrier_frequency: f32 = 1500.0;

    let message = Message::try_from(message_str).unwrap();

    println!("Message: {}", message.display_string);
    let message_bits_string = format!("{:077b}", message.message);
    println!("Message Bits: {}", message_bits_string);
    println!("Message Bits Len: {}", message_bits_string.len());

    let codeword = Ft8_Ldpc::from_message(message.message);

    println!("Crc: {:014b}", codeword.get_crc());
    println!("Parity: {:083b}", codeword.get_parity());

    let channel_symbols = modulation::channel_symbols::channel_symbols(codeword.get_codeword_bits());


    let channel_symbols_string:String = channel_symbols.iter().map(|b| (b + b'0') as char).collect();
    println!("Channel Symbols: {}", channel_symbols_string);

    let modulator = Modulator::new();
    let waveform = modulator.modulate(&channel_symbols, carrier_frequency);

    let mut samples:Vec<f32> = vec![0.0; (SAMPLE_RATE * 15.0) as usize];
    
    // Calculate noise standard deviation and power
    // let noise_db = 30.0;
    // let noise_sigma = (10.0_f32).powf(noise_db / 20.0);
    // let noise_power = 2500_f32 / SAMPLE_RATE * 2_f32 * noise_sigma * noise_sigma;  // Noise power in 2.5kHz
    // println!("noise_sigma: {}", noise_sigma);

    // // generate white noise
    let mut samples = generate_white_noise_s9(15 * SAMPLE_RATE as usize);

    // // Apply Band-Pass Filter (300 Hz – 2700 Hz)
    let mut samples = apply_bandpass_filter(&samples, SAMPLE_RATE as u32, LOW_CUTOFF_HZ, HIGH_CUTOFF_HZ);

    let noise_rms = rms_power(&samples);

    // // Calculate signal amplitude
    // // let snr = 0.0_f32;
    // // let tx_power = noise_power * 10_f32.powf(snr / 10_f32);
    // // let amplitude = (2.0_f32 * tx_power).sqrt();

    let starting_sample = ((0.5 + delta_time) * SAMPLE_RATE) as usize;

    mix_waveform(&mut samples, noise_rms, &waveform, starting_sample, -15.0);

    let i16_samples: Vec<i16> = samples.iter().map(|&sample| (sample * i16::MAX as f32) as i16).collect();

    let wavspec = WavSpec {
        channels: 1,
        sample_rate: 12000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int
    };
    let mut writer = WavWriter::create("plots/output.wav", wavspec).unwrap();
    for &sample in i16_samples.iter() {
        writer.write_sample(sample).unwrap();
    }
    println!("Wav file saved!");

    let spectrogram = generate_spectrogram(&samples);
    save_spectrogram_image(&spectrogram, "plots/spectrogram.png");
    println!("Spectrogram image saved!");

}

fn generate_spectrogram(audio_data: &[f32]) -> Vec<Vec<f32>> {
    let mut spectrogram =  Vec::new();

    let dtf_real_samples: usize = (SAMPLE_RATE / SYMBOL_RATE) as usize;
    let dft_window_size: usize = dtf_real_samples * 2;
    let time_step: usize = (SAMPLE_RATE / SYMBOL_RATE / 4.0) as usize;

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(dft_window_size);
    
    for start in (0..audio_data.len() - dtf_real_samples).step_by(time_step) {
        let window = &audio_data[start..start + dtf_real_samples];

        let mut buffer: Vec<Complex<f32>> = window
            .iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();
        buffer.resize(dft_window_size, Complex::new(0.0, 0.0));

        fft.process(&mut buffer);
        
        // Take only the first half of the FFT output (positive frequencies)
        let magnitudes: Vec<f32> = buffer
            .iter()
            .take(dft_window_size / 2)
            .map(|c| c.norm())
            .collect();

        spectrogram.push(magnitudes);
    }
    spectrogram
}

fn save_spectrogram_image(spectrogram: &[Vec<f32>], output_path: &str) {
    use plotly::{HeatMap, Plot};
    use plotly::common::{ColorScale, ColorScalePalette};

    // Define the maximum frequency to plot
    let max_frequency = 3000.0;
    let frequency_resolution = SAMPLE_RATE as f64 / (2.0 * spectrogram[0].len() as f64); // Frequency resolution
    let max_columns = (max_frequency / frequency_resolution).ceil() as usize;

    println!("{}, {}, {}", max_frequency, frequency_resolution, max_columns);

    // Convert spectrogram to a format suitable for plotting, truncating to max_columns
    let z: Vec<Vec<f64>> = spectrogram
        .iter().rev()
        .map(|row| row.iter().take(max_columns).map(|&val| val as f64).collect())
        .collect();

    // Generate x (frequency) and y (time) axes
    let x: Vec<f64> = (0..max_columns)
        .map(|i| i as f64 * frequency_resolution)
        .collect(); // Frequency bins

    let y: Vec<f64> = (0..spectrogram.len())
        .map(|i| i as f64)
        .collect(); // Time steps


    // // Convert spectrogram to a format suitable for plotting
    // let z: Vec<Vec<f64>> = spectrogram
    //     .iter()
    //     .map(|row| row.iter().map(|&val| val as f64).collect())
    //     .collect();

    // // Generate x (frequency) and y (time) axes
    // let x: Vec<f64> = (0..spectrogram[0].len())
    //     .map(|i| i as f64 * 3.125f64)
    //     .collect(); // Frequency bins

    // let y: Vec<f64> = (0..spectrogram.len())
    //     .map(|i| i as f64)
    //     .collect();

    // Create the heatmap
    let heatmap = HeatMap::new(x, y, z).color_scale(ColorScale::Palette(ColorScalePalette::Viridis));

    // Create the plot
    let mut plot = Plot::new();
    plot.add_trace(heatmap);

    // Save the plot as an image
    
    plot.write_image(output_path, plotly::ImageFormat::PNG, 1024, 768, 1.0);

}
