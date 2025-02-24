use libm::erf;
use std::f32::consts::PI;

pub fn modulate_mgfsk(symbols: &[u8], gaussian_boxcar: &[f32], tone_count: usize, symbol_rate: f32, sample_rate: f32) -> Vec<f32> {
    assert!(symbols.iter().all(|&s| s < tone_count as u8), "Symbol out of range");
    assert!(symbol_rate > 0.0, "symbol_rate must be > 0.0");
    assert!(sample_rate > 0.0, "sample_rate must be > 0.0");

    let samples_per_symbol = (sample_rate / symbol_rate) as usize;

    // pad the waveform with 2 extra symbols worth of samples at beginning and end
    let mut waveform = vec![0.0_f32; (symbols.len() + 4) * samples_per_symbol];
    
    // pad start and end of symbols with first and last symbol respectively
    let mut symbols = symbols.to_vec();
    symbols.insert(0, symbols[0]);
    symbols.push(symbols[symbols.len()-1]);
    let symbols = symbols;

    let mut s = 0;
    for symbol in symbols {
        for (i, sample) in gaussian_boxcar.iter().enumerate() {
            waveform[s+i] += sample * symbol as f32;
        }
        s += samples_per_symbol as usize;
    }

    // trim off the extra 4 symbols worth of samples and return
    return waveform[(samples_per_symbol * 2) as usize..waveform.len() - (samples_per_symbol * 2) as usize].to_vec();
}

pub fn gaussian_boxcar(bandwidth: f32, symbol_rate: f32, sample_rate: f32) -> Vec<f32> {
    let end = 1.5_f32;
    let start = -1.5_f32;
    let duration = end - start;
    let sample_count = (3.0 / symbol_rate * sample_rate) as usize;
    let step = duration / (sample_count as f32 - 1.0);
    let time_vector: Vec<f32> = (0..sample_count).map(|i| start + i as f32 * step).collect();

    let c = bandwidth * PI * (2.0 / f32::ln(2.0)).sqrt();
    let boxcar = time_vector.iter().map(|&ti| {0.5_f32 * (erf((c * (ti + 0.5)).into()) - erf((c * (ti - 0.5)).into())) as f32}).collect();
    return boxcar;
}

mod tests {
    use plotpy::{Curve, Plot, StrError};
    use super::{gaussian_boxcar, modulate_mgfsk};

    #[test]
    #[should_panic(expected = "symbol_rate must be > 0.0")]
    fn test_symbol_rate_gt_0() {
        let symbols = vec![0,1];
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, 12000.0);
        modulate_mgfsk(&symbols, &gaussian_boxcar, 8, 0.0, 12000.0);
    }

    #[test]
    #[should_panic(expected = "sample_rate must be > 0.0")]
    fn test_sample_rate_must_be_gt_0() {
        let symbols = vec![0,1];
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, 12000.0);
        modulate_mgfsk(&symbols, &gaussian_boxcar, 8, 6.25, 0.0);
    }

    #[test]
    #[should_panic(expected = "Symbol out of range")]
    fn test_all_symbols_must_be_less_than_tone_count() {
        let symbols = vec![0,10];
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, 12000.0);
        modulate_mgfsk(&symbols, &gaussian_boxcar, 8, 6.25, 12000.0);
    }

    #[test]
    fn test_should_return_expected_length() {
        let symbols = vec![0,1];
        let symbol_rate = 6.25;
        let sample_rate = 12000.0;
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, 12000.0);
        let samples = modulate_mgfsk(&symbols, &gaussian_boxcar, 8, symbol_rate, sample_rate);
        let expected_length = (1.0 / symbol_rate * symbols.len() as f32 * sample_rate) as usize;
        assert_eq!(samples.len(), expected_length);
    }

    #[test]
    fn test_should_not_return_large_frequency_jumps() {
        let symbols = vec![0,7];
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, 12000.0);
        let samples = modulate_mgfsk(&symbols, &gaussian_boxcar, 8, 6.25, 12000.0);
        
        let mut last_sample = samples[0];
        for sample in samples {
            let diff = (sample - last_sample).abs();
            assert!(diff < 6.25);
            last_sample = sample;
        }
    }

    #[test]
    fn test_plot_boxcar() -> Result<(), StrError> {
        let sample_rate = 12000_f32;
        let symbol_rate = 6.25_f32;

        let gaussian_boxcar = gaussian_boxcar(2.0, symbol_rate, sample_rate);
        
        let end = 1.5;
        let start = -1.5;
        let duration = end - start;
        let sample_count = (3.0 / symbol_rate * sample_rate) as usize;
        let step = duration / (sample_count as f32 - 1.0);
        let time_vector: Vec<f32> = (0..sample_count).map(|i| start + i as f32 * step).collect();

        let mut curve = Curve::new();
        curve.set_line_width(2.0);
        curve.points_begin();
        for (time,sample) in time_vector.iter().zip(gaussian_boxcar.iter()) {
            curve.points_add(time, &(sample));
        }
        curve.points_end();

        let mut plot = Plot::new();
        plot.set_title("Gaussian Filtered Boxcar Pulse");
        plot.add(&curve);

        plot.save("plots/gaussian_filtered_boxcar_pulse.png")?;

        Ok(())

    }

    #[test]
    fn test_plot_mgfsk() -> Result<(), StrError> {
        let sample_rate = 12000_f32;
        let symbol_rate = 6.25_f32;
        let tone_spacing = 6.25_f32;

        let symbols:Vec<u8> = vec![3,1,4,0,6,5,2];
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, 12000.0);
        let samples = modulate_mgfsk(&symbols, &gaussian_boxcar, 8, symbol_rate, sample_rate);
        let time_vector:Vec<f32> = (0..samples.len()).map(|i: usize| i as f32 / sample_rate ).collect();

        let mut curve = Curve::new();
        curve.set_line_width(2.0);
        curve.points_begin();
        for (time,sample) in time_vector.iter().zip(samples.iter()) {
            curve.points_add(time, &(sample * tone_spacing));
        }
        curve.points_end();

        let mut plot = Plot::new();
        plot.set_title("M-GFSK Modulation Waveform");
        plot.add(&curve).grid_and_labels("Time (seconds)", "Frequency Shift (hertz)");

        plot.save("plots/mgfsk_modulation_waveform.png")?;

        Ok(())

    }
}