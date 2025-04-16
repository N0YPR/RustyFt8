use std::f32::consts::PI;

pub fn continuous_phase_frequency_modulation(samples: &[f32], carrier_frequency: f32, sample_rate: f32, tone_spacing_hz:f32) -> Vec<f32> {
    assert!(samples.iter().all(|&x| x >= 0.0), "Input data must be >= 0.0");
    let two_pi = 2.0 * PI;
    let cycle = two_pi / sample_rate;
    let dphase_symbol = cycle * tone_spacing_hz;
    let dphase_carrier_frequency = cycle * carrier_frequency;

    let mut phase = 0.0;
    let mut sample = 0;
    
    let waveform:Vec<f32> = samples.iter().map(|&symbol| {
        let delta_phase = symbol * dphase_symbol + dphase_carrier_frequency;
        phase += delta_phase;
        phase %= two_pi;
        sample += 1;
        return phase.sin();
    }).collect();

    return waveform;

}

mod tests {
    use plotpy::{Curve, Plot, StrError};
    use super::*;
    use crate::modulation::mgfsk::*;

    #[test]
    fn test_plot_cpfm() -> Result<(), StrError> {
        let sample_rate = 12000_f32;
        let symbol_rate = 6.25_f32;
        let tone_spacing = 6.25_f32;

        let symbols:Vec<u8> = vec![3,1,4,0,6,5,2];
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, 12000.0);
        let samples = modulate_mgfsk(&symbols, &gaussian_boxcar, 8, symbol_rate, sample_rate);
        let time_vector:Vec<f32> = (0..samples.len()).map(|i: usize| i as f32 / sample_rate ).collect();

        let waveform = continuous_phase_frequency_modulation(&samples, 10.0, sample_rate, tone_spacing);

        let mut curve = Curve::new();
        curve.set_line_width(2.0);
        curve.points_begin();
        for (time,sample) in time_vector.iter().zip(waveform.iter()) {
            curve.points_add(time, sample);
        }
        curve.points_end();

        let mut plot = Plot::new();
        plot.set_title("Frequency Modulated Carrier");
        plot.add(&curve).grid_and_labels("Time (seconds)", "Volts");

        plot.save("plots/cpfm_waveform.png")?;

        Ok(())

    }

    #[test]
    fn test_phase_continuity() {
        use std::f32::consts::PI;

        let data = vec![0.1, 0.2, 0.3, 0.2, 0.1, 0.15, 0.25, 0.3, 0.2, 0.1]; // Example smoothly varying data
        let sample_rate = 48000.0;
        let freq_deviation = 500.0;
        let carrier_freq = 5000.0;
        
        let modulated_signal = continuous_phase_frequency_modulation(&data, sample_rate, freq_deviation, carrier_freq);

        let mut prev_phase = modulated_signal[0].atan2(1.0); // Initial phase reference

        for &sample in modulated_signal.iter().skip(1) {
            let phase = sample.atan2(1.0); // Compute current phase
            let phase_diff = (phase - prev_phase).abs(); 

            // Ensure phase difference is small (no abrupt jumps)
            assert!(phase_diff < PI / 2.0, "Phase discontinuity detected!");

            prev_phase = phase; // Update phase for next iteration
        }
    }
}