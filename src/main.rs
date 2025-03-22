use std::env;

use constants::SAMPLE_RATE;
use error_correction::ldpc::Ft8_Ldpc;
use hound::{WavSpec, WavWriter};
use message::Message;
use modulation::Modulator;
use rustfft::{num_complex::Complex, FftPlanner};
use simulation::noise::{apply_bandpass_filter, generate_white_noise_s9, mix_waveform, rms_power, HIGH_CUTOFF_HZ, LOW_CUTOFF_HZ};
use sonogram::{ColourGradient, FrequencyScale, SpecOptionsBuilder};
use std::time::Instant;
use plotly::common::Mode;
use plotly::{Plot, Scatter};

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

    // // let mut carrier_frequency = 500.0;
    // // while carrier_frequency < 2800.0 {
    // //     let msg_str = format!("HZ{}", carrier_frequency);
    // //     println!("{}", msg_str);
    // //     let message = Message::try_from(msg_str).unwrap();
    // //     let waveform = modulator.modulate(&message.channel_symbols, carrier_frequency);
    // //     mix_waveform(&mut samples, noise_rms, &waveform, starting_sample, -10.0);
    // //     carrier_frequency += 100.0;
    // // }
    // // normalize_signal(&mut samples);

    // // // Apply QSB to our signal (Slow Fading)
    // // //let waveform = apply_qsb(&waveform, SAMPLE_RATE as u32, QSB_FREQ_HZ);

    // // // Introduce weak carrier-like fluttering to our signal
    // // //let waveform = apply_fluttering(&waveform, SAMPLE_RATE as u32, FLUTTER_FREQ_HZ);

    // // // Calculate noise standard deviation and power
    // // let noise_db = 30.0;
    // // let noise_sigma = (10.0_f32).powf(noise_db / 20.0);
    // // let noise_power = 2500_f32 / SAMPLE_RATE * 2_f32 * noise_sigma * noise_sigma;  // Noise power in 2.5kHz

    // // // generate white noise
    // // let mut signal = generate_white_noise(15 * SAMPLE_RATE as usize, noise_sigma);

    // // // Calculate signal amplitude
    // // let snr = -10_f32;
    // // let tx_power = noise_power * 10_f32.powf(snr / 10_f32);
    // // let amplitude = (2.0_f32 * tx_power).sqrt();

    // // // add our waveform
    // // //let amplitude = 0.25;
    // // let starting_sample = ((0.5 + delta_time) * SAMPLE_RATE) as usize;
    // // for i in 0..signal.len() {
    // //     if i < starting_sample {
    // //         continue;
    // //     }

    // //     if i >= waveform.len() {
    // //         break;
    // //     }

    // //     signal[i] = signal[i] + waveform[i] * amplitude;
    // // }


    let i16_samples: Vec<i16> = samples.iter().map(|&sample| (sample * i16::MAX as f32) as i16).collect();

    
    let wavspec = WavSpec {
        channels: 1,
        sample_rate: 12000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int
    };
    let mut writer = WavWriter::create("output.wav", wavspec).unwrap();

    // for &sample in &samples {
    //     let int_sample = (sample * i16::MAX as f32) as i16;
    //     writer.write_sample(int_sample).unwrap();
    // }
    for &sample in i16_samples.iter() {
        writer.write_sample(sample).unwrap();
    }
    
    // Calculate the spectrogram
    let start = Instant::now();
    let spectrogram = calculate_spectrogram(&samples, 1920);
    let duration = start.elapsed();

    println!("Time taken to calculate spectrogram: {:?}", duration);

    let start_plot = Instant::now();
    plot_spectrogram(&spectrogram);
    let duration_plot = start_plot.elapsed();
    println!("Time taken to plot spectrogram: {:?}", duration_plot);
}


fn calculate_spectrogram(data: &[f32], samples_per_symbol: usize) -> Vec<Vec<f32>> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(samples_per_symbol * 2);

    let mut spectrogram = Vec::new();
    let step_size = samples_per_symbol / 4;

    for start in (0..data.len() - samples_per_symbol).step_by(step_size) {
        let mut window: Vec<Complex<f32>> = data[start..start + samples_per_symbol]
            .iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();

        // Zero padding
        window.resize(samples_per_symbol * 2, Complex::new(0.0, 0.0));

        // Perform FFT
        fft.process(&mut window);

        // Calculate power spectrum
        let power_spectrum: Vec<f32> = window.iter().map(|c| c.norm_sqr()).collect();
        spectrogram.push(power_spectrum);
    }

    spectrogram
}

fn plot_spectrogram(spectrogram: &[Vec<f32>]) {
    let mut plot = Plot::new();
    
    let z: Vec<Vec<f64>> = spectrogram
        .iter()
        .map(|row| row.iter().map(|&val| val as f64).collect())
        .collect();

    let trace = plotly::HeatMap::new(
        (0..spectrogram.len()).collect::<Vec<_>>(),
        (0..spectrogram[0].len()).collect::<Vec<_>>(),
        z,
    );

    plot.add_trace(trace);

    plot.write_image("output.png", plotly::ImageFormat::PNG, 800, 600, 1.0);
}