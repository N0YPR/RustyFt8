use amplitude::amplitude_shaping;
use cpfm::continuous_phase_frequency_modulation;
use mgfsk::{gaussian_boxcar, modulate_mgfsk};

use crate::constants::*;

mod amplitude;
pub mod channel_symbols;
mod cpfm;
mod mfsk;
mod mgfsk;

pub struct Modulator {
    boxcar_pulse: Vec<f32>
}

impl Modulator {
    pub fn new() -> Self {
        
        let boxcar_pulse = gaussian_boxcar(2.0, SYMBOL_RATE, SAMPLE_RATE);

        Modulator {
            boxcar_pulse
        }
    }

    pub fn modulate(&self, symbols: &[u8], carrier_frequency: f32) -> Vec<f32> {
        let frequencies = modulate_mgfsk(&symbols, &self.boxcar_pulse, TONE_COUNT, SYMBOL_RATE, SAMPLE_RATE);
        let waveform = continuous_phase_frequency_modulation(&frequencies, carrier_frequency, SAMPLE_RATE, TONE_SPACING);
        let amplitude_shaped_waveform = amplitude_shaping(&waveform, SAMPLE_RATE, SYMBOL_RATE);

        amplitude_shaped_waveform
    }
}

mod tests {
    use hound::{WavSpec, WavWriter};

    use super::*;

    #[test]
    fn blah() {
        let symbols_str = "3140652567536417506116571667463525453140652463211417534323007747355225123140652";
        let symbols:Vec<u8> = symbols_str.chars().map(|c| c as u8 - b'0').collect();

        let modulator = Modulator::new();

        let waveform = modulator.modulate(&symbols, 1000.0);

        let wavspec = WavSpec {
            channels: 1,
            sample_rate: 12000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int
        };
        let mut writer = WavWriter::create("/tmp/output.wav", wavspec).unwrap();

        for &sample in &waveform {
            let int_sample = (sample * i16::MAX as f32) as i16;
            writer.write_sample(int_sample).unwrap();
        }
    }
}