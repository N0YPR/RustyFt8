#![allow(unused)]

pub fn modulate_mfsk(
    symbols: &[u8],
    tone_count: u8,
    symbol_rate: f64,
    sample_rate: f64,
) -> Vec<f64> {
    assert!(
        symbols.iter().all(|&s| s < tone_count),
        "Symbol out of range"
    );
    assert!(symbol_rate > 0.0, "symbol_rate must be > 0.0");
    assert!(sample_rate > 0.0, "sample_rate must be > 0.0");

    let symbol_duration = 1.0 / symbol_rate;
    let samples_per_symbol = (symbol_duration * sample_rate) as usize;

    let mut samples = vec![];

    for symbol in symbols {
        samples.resize(samples.len() + samples_per_symbol, *symbol as f64);
    }

    return samples;
}

mod tests {
    use super::modulate_mfsk;
    use plotpy::{Curve, Plot, StrError};

    #[test]
    #[should_panic(expected = "symbol_rate must be > 0.0")]
    fn test_symbol_rate_gt_0() {
        let symbols = vec![0, 1];
        modulate_mfsk(&symbols, 8, 0.0, 12000.0);
    }

    #[test]
    #[should_panic(expected = "sample_rate must be > 0.0")]
    fn test_sample_rate_must_be_gt_0() {
        let symbols = vec![0, 1];
        modulate_mfsk(&symbols, 8, 6.25, 0.0);
    }

    #[test]
    #[should_panic(expected = "Symbol out of range")]
    fn test_all_symbols_must_be_less_than_tone_count() {
        let symbols = vec![0, 10];
        modulate_mfsk(&symbols, 8, 6.25, 12000.0);
    }

    #[test]
    fn test_should_return_expected_length() {
        let symbols = vec![0, 1];
        let symbol_rate = 6.25;
        let sample_rate = 12000.0;
        let samples = modulate_mfsk(&symbols, 8, symbol_rate, sample_rate);
        let expected_length = (1.0 / symbol_rate * symbols.len() as f64 * sample_rate) as usize;
        assert_eq!(samples.len(), expected_length);
    }

    #[test]
    fn test_plot() -> Result<(), StrError> {
        let sample_rate = 12000_f64;
        let symbol_rate = 6.25_f64;
        let tone_spacing = 6.25_f64;

        let symbols: Vec<u8> = vec![3, 1, 4, 0, 6, 5, 2];
        let samples = modulate_mfsk(&symbols, 8, symbol_rate, sample_rate);
        let time_vector: Vec<f64> = (0..samples.len())
            .map(|i: usize| i as f64 / sample_rate)
            .collect();

        let mut curve = Curve::new();
        curve.set_line_width(2.0);
        curve.points_begin();
        for (time, sample) in time_vector.iter().zip(samples.iter()) {
            curve.points_add(time, &(sample * tone_spacing));
        }
        curve.points_end();

        let mut plot = Plot::new();
        plot.set_title("M-FSK Modulation Waveform");
        plot.add(&curve)
            .grid_and_labels("Time (seconds)", "Frequency Shift (hertz)");

        plot.save("plots/mfsk_modulation_waveform.png")?;

        Ok(())
    }
}
