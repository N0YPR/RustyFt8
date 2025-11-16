///! Downsampling utilities for FT8 signal processing
///!
///! Downsamples the signal to ~200 Hz centered on a specific frequency.

extern crate alloc;
use alloc::vec;
use alloc::string::String;
use super::fft::{fft_complex, fft_complex_inverse};
use super::{SAMPLE_RATE, NMAX, NSPS};

/// Downsample signal to ~200 Hz centered at f0
///
/// Uses FFT-based bandpass filtering and decimation.
///
/// # Arguments
/// * `signal` - Input signal at 12 kHz
/// * `f0` - Center frequency in Hz
/// * `output` - Output buffer for downsampled complex signal (4096 samples)
///
/// # Returns
/// Actual output sample rate in Hz
pub fn downsample_200hz(
    signal: &[f32],
    f0: f32,
    output: &mut [(f32, f32)],
) -> Result<f32, String> {
    const NFFT_IN: usize = 262144; // Large FFT (2^18) for high resolution
    const NFFT_OUT: usize = 4096;   // Power of 2 for FFT

    if signal.len() < NMAX {
        return Err(alloc::format!("Signal too short: {}", signal.len()));
    }

    if output.len() < 4096 {
        return Err(alloc::format!("Output buffer too small: {} (need at least 4096)", output.len()));
    }

    // Allocate FFT buffers
    let mut x_real = vec![0.0f32; NFFT_IN];
    let mut x_imag = vec![0.0f32; NFFT_IN];

    // Copy signal and zero-pad
    for i in 0..NMAX {
        x_real[i] = signal[i];
        x_imag[i] = 0.0;
    }

    // Forward FFT
    fft_complex(&mut x_real, &mut x_imag, NFFT_IN)?;

    // Extract bandwidth around f0
    // FT8 bandwidth: 8 * baud = 8 * 6.25 = 50 Hz
    // Extract f0 ± 5 Hz (10 Hz total) at 200 Hz sampling = 62.5 bins
    let df = SAMPLE_RATE / NFFT_IN as f32;
    let baud = SAMPLE_RATE / NSPS as f32; // 6.25 Hz

    // Frequency range to extract: [f0 - 1.5*baud, f0 + 8.5*baud]
    let fb = (f0 - 1.5 * baud).max(0.0);
    let ft = (f0 + 8.5 * baud).min(SAMPLE_RATE / 2.0);

    // Use rounding (not truncation) to match WSJT-X nint()
    let ib = (fb / df).round().max(1.0) as usize;
    let it = (ft / df).round().min((NFFT_IN / 2) as f32) as usize;
    let i0 = (f0 / df).round() as usize;

    // Copy selected frequency bins to output FFT buffer
    let mut out_real = vec![0.0f32; NFFT_OUT];
    let mut out_imag = vec![0.0f32; NFFT_OUT];

    let mut k = 0;
    for i in ib..=it {
        if k < NFFT_OUT {
            out_real[k] = x_real[i];
            out_imag[k] = x_imag[i];
            k += 1;
        }
    }

    // Calculate actual output sample rate
    let bandwidth = (it - ib + 1) as f32 * df;
    let actual_sample_rate = bandwidth * (NFFT_OUT as f32) / (k as f32);

    // Apply taper to edges
    let taper_len = 101;
    for i in 0..taper_len {
        let taper_val = 0.5 * (1.0 + libm::cosf(core::f32::consts::PI * i as f32 / 100.0));
        if i < k {
            out_real[i] *= taper_val;
            out_imag[i] *= taper_val;
        }
        let j = k - 1 - i;
        if j < k {
            out_real[j] *= taper_val;
            out_imag[j] *= taper_val;
        }
    }

    // Circular shift to center at DC (matching WSJT-X cshift operation)
    let shift = (i0 as i32 - ib as i32).max(0) as usize;

    if shift > 0 && shift < k {
        // Rotate array left by 'shift' positions
        let mut temp_real = vec![0.0f32; NFFT_OUT];
        let mut temp_imag = vec![0.0f32; NFFT_OUT];
        temp_real.copy_from_slice(&out_real);
        temp_imag.copy_from_slice(&out_imag);

        for i in 0..k {
            let src = (i + shift) % k;
            out_real[i] = temp_real[src];
            out_imag[i] = temp_imag[src];
        }
    }

    // Inverse FFT
    fft_complex_inverse(&mut out_real, &mut out_imag, NFFT_OUT)?;

    // Normalize and copy to output
    let fac = 1.0 / libm::sqrtf((NFFT_IN * NFFT_OUT) as f32);
    for i in 0..NFFT_OUT {
        output[i] = (out_real[i] * fac, out_imag[i] * fac);
    }

    Ok(actual_sample_rate)
}
