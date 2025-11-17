//! FT8 Signal Subtraction
//!
//! Removes decoded signals from audio to reveal weaker masked signals.
//! Uses matched filtering to estimate signal amplitude, then reconstructs and subtracts.
//!
//! Algorithm (from WSJT-X subtractft8.f90):
//! 1. Generate complex reference signal from decoded tones
//! 2. Mix with audio: camp(t) = audio(t) * conj(cref(t))
//! 3. Low-pass filter: cfilt(t) = LPF[camp(t)]
//! 4. Reconstruct: signal(t) = 2 * Re{cref(t) * cfilt(t)}
//! 5. Subtract from audio

use crate::pulse;
use rustfft::{FftPlanner, num_complex::Complex};

const SAMPLE_RATE: f32 = 12000.0;
const NSPS: usize = 1920;
const NFRAME: usize = 79 * NSPS; // 151,680 samples
const NFILT: usize = 4000; // Filter length

/// Low-pass filter for signal amplitude estimation
///
/// Uses FFT-based convolution with cosine-squared window
struct LowPassFilter {
    /// FFT of filter kernel in frequency domain
    filter_kernel: Vec<Complex<f32>>,
    /// FFT planner
    fft_planner: FftPlanner<f32>,
    /// End correction factors for window edge effects
    end_correction: Vec<f32>,
    /// Buffer size (power of 2)
    nfft: usize,
}

impl LowPassFilter {
    /// Create a new low-pass filter
    fn new(nfilt: usize, nfft: usize) -> Result<Self, String> {
        if !nfft.is_power_of_two() {
            return Err(format!("FFT size must be power of 2, got {}", nfft));
        }

        use core::f32::consts::PI;

        // Create and normalize the cosine-squared window
        let mut window = vec![0.0f32; nfilt + 1];
        let mut sumw = 0.0;

        for (j, w) in window.iter_mut().enumerate() {
            let idx = j as i32 - (nfilt as i32 / 2);
            *w = f32::cos(PI * idx as f32 / nfilt as f32).powi(2);
            sumw += *w;
        }

        // Normalize window
        for w in window.iter_mut() {
            *w /= sumw;
        }

        // Create filter kernel (circularly shifted window)
        let mut filter_kernel = vec![Complex::new(0.0, 0.0); nfft];
        for j in 0..=nfilt {
            let idx = (j + nfft - nfilt / 2) % nfft;
            filter_kernel[idx] = Complex::new(window[j], 0.0);
        }

        // FFT the filter kernel
        let mut fft_planner = FftPlanner::new();
        let fft = fft_planner.plan_fft_forward(nfft);
        fft.process(&mut filter_kernel);

        // Normalize by FFT size
        let fac = 1.0 / nfft as f32;
        for k in filter_kernel.iter_mut() {
            *k *= fac;
        }

        // Compute end correction factors
        // WSJT-X: endcorrection(j) = 1.0/(1.0-sum(window(j-1:NFILT/2))/sumw)
        // Our window is 0..nfilt, centered at nfilt/2, already normalized
        // So we sum from (nfilt/2 + j-1) to nfilt
        let mut end_correction = vec![1.0f32; nfilt / 2 + 1];
        for j in 1..=nfilt / 2 {
            let start_idx = nfilt / 2 + (j - 1);
            let sum: f32 = window[start_idx..=nfilt].iter().sum();
            end_correction[j] = 1.0 / (1.0 - sum);
        }

        Ok(Self {
            filter_kernel,
            fft_planner,
            end_correction,
            nfft,
        })
    }

    /// Apply low-pass filter to complex signal
    ///
    /// # Arguments
    /// * `signal` - Input complex signal (will be modified in place)
    /// * `signal_len` - Actual signal length (may be < buffer size)
    fn apply(&mut self, signal: &mut [Complex<f32>], signal_len: usize) -> Result<(), String> {
        if signal.len() < self.nfft {
            return Err(format!("Signal buffer too small: {} < {}", signal.len(), self.nfft));
        }

        // Zero-pad the rest of the buffer
        for s in signal[signal_len..self.nfft].iter_mut() {
            *s = Complex::new(0.0, 0.0);
        }

        // Forward FFT
        let fft_fwd = self.fft_planner.plan_fft_forward(self.nfft);
        fft_fwd.process(signal);

        // Multiply by filter kernel in frequency domain
        for (sig, filt) in signal[..self.nfft].iter_mut().zip(self.filter_kernel.iter()) {
            *sig *= *filt;
        }

        // Inverse FFT
        let fft_inv = self.fft_planner.plan_fft_inverse(self.nfft);
        fft_inv.process(signal);

        // Note: RustFFT doesn't normalize IFFT by default, but we already normalized
        // the filter kernel by 1/nfft, so the convolution result has the correct scale

        // Apply end corrections
        for j in 0..self.end_correction.len() {
            if j < signal_len {
                signal[j] *= self.end_correction[j];
            }

            let back_idx = signal_len.saturating_sub(j + 1);
            if back_idx < signal_len {
                signal[back_idx] *= self.end_correction[j];
            }
        }

        Ok(())
    }
}

/// Subtract a decoded FT8 signal from audio
///
/// # Arguments
/// * `audio` - Input/output audio buffer (modified in place)
/// * `tones` - Decoded tone sequence (79 symbols, values 0-7)
/// * `frequency` - Signal carrier frequency in Hz
/// * `time_offset` - Signal time offset in seconds
///
/// # Returns
/// * `Ok(())` on success
/// * `Err(msg)` on error
pub fn subtract_ft8_signal(
    audio: &mut [f32],
    tones: &[u8; 79],
    frequency: f32,
    time_offset: f32,
) -> Result<(), String> {
    // Generate complex reference signal
    let mut pulse_buf = vec![0.0f32; 3 * NSPS];
    pulse::compute_pulse(&mut pulse_buf, pulse::BT, NSPS)?;

    let mut cref = vec![(0.0f32, 0.0f32); NFRAME];
    pulse::generate_complex_waveform(
        tones,
        &mut cref,
        &pulse_buf,
        frequency,
        SAMPLE_RATE,
        NSPS,
    )?;

    // Calculate start position in audio (can be negative)
    let nstart = (time_offset * SAMPLE_RATE) as i32;

    // Initialize filter
    let nfft = audio.len().next_power_of_two().max(NFRAME.next_power_of_two());
    let mut filter = LowPassFilter::new(NFILT, nfft)?;

    // Mix audio with conjugate of reference to get complex amplitude
    let mut camp = vec![Complex::new(0.0, 0.0); nfft];

    for i in 0..NFRAME {
        // Handle negative start position
        let j_signed = nstart + i as i32;
        if j_signed >= 0 && (j_signed as usize) < audio.len() {
            let j = j_signed as usize;
            // camp[i] = audio[j] * conj(cref[i])
            camp[i] = Complex::new(
                audio[j] * cref[i].0,  // Real part: audio * cos
                -audio[j] * cref[i].1, // Imag part: -audio * sin (conjugate)
            );
        }
    }

    // Low-pass filter to get smoothed amplitude estimate
    filter.apply(&mut camp, NFRAME)?;

    // Reconstruct and subtract the signal
    for i in 0..NFRAME {
        // Handle negative start position
        let j_signed = nstart + i as i32;
        if j_signed >= 0 && (j_signed as usize) < audio.len() {
            let j = j_signed as usize;
            // reconstructed = 2 * Re{cref[i] * camp[i]}
            // Complex multiplication: (a+bi)(c+di) = (ac-bd) + (ad+bc)i
            let re = cref[i].0;  // a = cos(phi)
            let im = cref[i].1;  // b = sin(phi)
            let camp_re = camp[i].re; // c
            let camp_im = camp[i].im; // d

            let reconstructed = 2.0 * (re * camp_re - im * camp_im);

            // Subtract from audio
            audio[j] -= reconstructed;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowpass_filter_creation() {
        let result = LowPassFilter::new(4000, 16384);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lowpass_filter_invalid_nfft() {
        let result = LowPassFilter::new(4000, 12345); // Not power of 2
        assert!(result.is_err());
    }

    #[test]
    fn test_subtract_reduces_signal() {
        // Create a synthetic FT8 signal
        let tones = [3u8; 79]; // All tone 3

        // Generate clean signal
        let mut pulse_buf = vec![0.0f32; 3 * 1920];
        pulse::compute_pulse(&mut pulse_buf, 2.0, 1920).unwrap();

        let mut clean_signal = vec![0.0f32; 79 * 1920];
        pulse::generate_waveform(
            &tones,
            &mut clean_signal,
            &pulse_buf,
            1500.0,
            12000.0,
            1920,
        ).unwrap();

        // Create audio with signal embedded
        let audio_len = 15 * 12000; // 15 seconds
        let mut audio = vec![0.0f32; audio_len];

        // Place signal at 0.5 seconds
        let start = (0.5 * 12000.0) as usize;
        for (i, &sample) in clean_signal.iter().enumerate() {
            if start + i < audio_len {
                audio[start + i] = sample;
            }
        }

        // Measure power before subtraction
        let power_before: f32 = audio[start..start + clean_signal.len()]
            .iter()
            .map(|&x| x * x)
            .sum();

        // Subtract the signal (time_offset matches where signal was placed)
        let result = subtract_ft8_signal(&mut audio, &tones, 1500.0, 0.5);
        assert!(result.is_ok());

        // Measure power after subtraction
        let power_after: f32 = audio[start..start + clean_signal.len()]
            .iter()
            .map(|&x| x * x)
            .sum();

        // Power should be significantly reduced (>20 dB reduction)
        let reduction_db = 10.0 * (power_after / power_before).log10();
        println!("Signal power reduction: {:.1} dB", reduction_db);
        assert!(
            reduction_db < -20.0,
            "Expected >20 dB reduction (negative value), got {:.1} dB",
            reduction_db
        );
    }
}
