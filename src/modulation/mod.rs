use amplitude::amplitude_shaping;
use cpfm::continuous_phase_frequency_modulation;
use mgfsk::{gaussian_boxcar, modulate_mgfsk};

use crate::constants::*;

mod amplitude;
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
        //let symbols_str = "3140652567536417506116571667463525453140652463211417534323007747355225123140652";
        //let symbols_str = "3140652035544254712706252727463530023140652104324543702156402641651037413140652";
        //let symbols_str = "3140652567536417506116571667426331463140652425052324153532606106456567713140652";
        let symbols_str = "3140652035544254712706252717456036763140652351556560450306541031517673073140652";
        let mut symbols:Vec<u8> = symbols_str.chars().map(|c| c as u8 - b'0').collect();
        //let symbols:Vec<u8> = vec![3,1,4,0,6,5,2,0,0,0,0,0,0,0,0,1,0,0,6,1,1,6,5,7,1,6,5,2,1,7,5,5,2,6,1,4,3,1,4,0,6,5,2,5,7,6,5,1,1,3,1,1,1,4,1,1,5,0,2,3,3,1,2,7,6,7,4,2,4,1,6,6,3,1,4,0,6,5,2];
        //let symbols:Vec<u8> = vec![3,1,4,0,6,5,2,0,3,5,5,4,4,2,5,4,7,1,2,7,0,6,2,5,2,7,0,3,2,2,4,4,2,5,6,6,3,1,4,0,6,5,2,6,6,6,2,3,5,3,1,4,0,5,1,1,3,5,6,6,7,1,1,4,5,5,1,1,3,2,3,1,3,1,4,0,6,5,2];
        
        let modulator = Modulator::new();

        let waveform = modulator.modulate(&symbols, 1000.0);

        let wavspec = WavSpec {
            channels: 1,
            sample_rate: 12000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int
        };
        let mut writer = WavWriter::create("output.wav", wavspec).unwrap();

        for &sample in &waveform {
            let int_sample = (sample * i16::MAX as f32) as i16;
            writer.write_sample(int_sample).unwrap();
        }

        //assert!(false);
    }
}