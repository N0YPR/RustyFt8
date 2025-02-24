use std::f32::consts::PI;

pub fn amplitude_shaping(samples: &[f32], sample_rate: f32, symbol_rate:f32) -> Vec<f32> {
    let symbol_duration = 1.0 / symbol_rate;
    let ramp_duration = symbol_duration / 8.0;
    let ramp_samples = (sample_rate * ramp_duration) as usize;
    let step = 1.0 / sample_rate;
    let mut samples = samples.to_vec();
    let mut t = 0.0;

    for i in 0..ramp_samples {
        let j = samples.len() - 1 - i;
        samples[i] = samples[i] * 0.5 * (1.0 - (8.0 * PI * t / symbol_duration).cos());
        samples[j] = samples[j] * 0.5 * (1.0 - (8.0 * PI * t / symbol_duration).cos());
        t += step;
    }

    let samples = samples;
    return samples;
}

mod tests {
    use plotpy::{Curve, Plot, StrError};
    use super::*;
    use crate::modulation::cpfm::*;
    use crate::modulation::mgfsk::*;

    #[test]
    fn test_plot_amplitude() -> Result<(), StrError> {
        let sample_rate = 48000_f32;
        let symbol_rate = 6.25_f32;
        let tone_spacing = 6.25_f32;

        let symbols:Vec<u8> = vec![3,1,4,0,6,5,2];
        let gaussian_boxcar = gaussian_boxcar(2.0, 6.25, sample_rate);
        let samples = modulate_mgfsk(&symbols, &gaussian_boxcar, 8, symbol_rate, sample_rate);
        let time_vector:Vec<f32> = (0..samples.len()).map(|i: usize| i as f32 / sample_rate ).collect();

        let waveform = continuous_phase_frequency_modulation(&samples, 1000.0, sample_rate, tone_spacing);
        let waveform = amplitude_shaping(&waveform, sample_rate, symbol_rate);

        let mut curve_start = Curve::new();
        curve_start.set_line_width(2.0);
        curve_start.points_begin();
        for (time,sample) in time_vector[0..2000].iter().zip(waveform[0..2000].iter()) {
            curve_start.points_add(time, sample);
        }
        curve_start.points_end();

        let mut curve_end = Curve::new();
        curve_end.set_line_width(2.0);
        curve_end.points_begin();
        for (time,sample) in time_vector[time_vector.len()-2000..].iter().zip(waveform[waveform.len()-2000..].iter()) {
            curve_end.points_add(time, sample);
        }
        curve_end.points_end();

        let mut plot = Plot::new();
        plot.set_super_title("Amplitude Shaped Waveform", None);

        plot.set_subplot(1, 2, 1)
            .set_title("Start")
            .add(&curve_start)
            .grid_and_labels("Time (seconds)", "Volts");

        plot.set_subplot(1, 2, 2)
            .set_title("End")
            .add(&curve_end)
            .grid_and_labels("Time (seconds)", "Volts");

        plot.save("plots/amplitude_shaped_waveform.png")?;

        Ok(())

    }
}