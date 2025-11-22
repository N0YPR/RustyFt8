///! Downsampling utilities for FT8 signal processing
///!
///! Downsamples the signal to ~200 Hz centered on a specific frequency.

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
    // Match WSJT-X ft8_downsample.f90 exactly:
    // NFFT1=192000, NFFT2=3200
    // This gives exactly 200 Hz output sample rate (12000/60 = 200)
    // With 32-point FFT: 200/32 = 6.25 Hz per bin (perfect match to FT8 tone spacing!)
    const NFFT_IN: usize = 192000;  // Was 262144
    const NFFT_OUT: usize = 3200;   // Was 4096

    if signal.len() < NMAX {
        return Err(format!("Signal too short: {}", signal.len()));
    }

    if output.len() < NFFT_OUT {
        return Err(format!("Output buffer too small: {} (need at least {})", output.len(), NFFT_OUT));
    }

    // Allocate FFT buffers
    let mut x_real = vec![0.0f32; NFFT_IN];
    let mut x_imag = vec![0.0f32; NFFT_IN];

    // Copy signal (limited to NFFT_IN) and zero-pad
    let copy_len = NFFT_IN.min(NMAX);
    for i in 0..copy_len {
        x_real[i] = signal[i];
        x_imag[i] = 0.0;
    }
    // Zero-pad the rest if needed
    for i in copy_len..NFFT_IN {
        x_real[i] = 0.0;
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

    // eprintln!("DOWNSAMPLE: f0={:.1} Hz, df={:.3} Hz, fb={:.1} Hz, ft={:.1} Hz, ib={}, it={}, i0={}",
    //           f0, df, fb, ft, ib, it, i0);

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

    // eprintln!("  Copied {} bins, bandwidth={:.1} Hz", k, (it - ib + 1) as f32 * df);

    // Check power in copied bins
    // let bin_power: f32 = out_real.iter().take(k).zip(out_imag.iter().take(k))
    //     .map(|(r, i)| r*r + i*i).sum();
    // eprintln!("  Power in copied bins: {:.3}", bin_power);

    // Calculate actual output sample rate
    let bandwidth = (it - ib + 1) as f32 * df;
    let actual_sample_rate = bandwidth * (NFFT_OUT as f32) / (k as f32);

    // Apply taper to edges
    let taper_len = 101;
    for i in 0..taper_len {
        let taper_val = 0.5 * (1.0 + f32::cos(core::f32::consts::PI * i as f32 / 100.0));
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
    // eprintln!("  Circular shift: shift={} (i0={}, ib={})", shift, i0, ib);

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

        // Check power after shift
        // let shift_power: f32 = out_real.iter().take(k).zip(out_imag.iter().take(k))
        //     .map(|(r, i)| r*r + i*i).sum();
        // eprintln!("  Power after shift: {:.3}", shift_power);
    }

    // Inverse FFT
    fft_complex_inverse(&mut out_real, &mut out_imag, NFFT_OUT)?;

    // Check power after inverse FFT
    // let ifft_power: f32 = out_real.iter().take(100).zip(out_imag.iter().take(100))
    //     .map(|(r, i)| r*r + i*i).sum();
    // eprintln!("  Power after IFFT (first 100): {:.3}", ifft_power);

    // Normalize and copy to output
    // Match WSJT-X: fac = 1.0/sqrt(float(NFFT1)*NFFT2)
    // BUT: Our IFFT already divided by NFFT_OUT, so we need to multiply it back first
    // fft_complex_inverse divides by N=3200 (see fft.rs:113)
    // So effective normalization = NFFT_OUT / sqrt(NFFT_IN * NFFT_OUT) = sqrt(NFFT_OUT / NFFT_IN)
    let fac = (NFFT_OUT as f32 / NFFT_IN as f32).sqrt();
    // eprintln!("  Normalization factor: {:.6} (accounts for IFFT scaling)", fac);
    for i in 0..NFFT_OUT {
        output[i] = (out_real[i] * fac, out_imag[i] * fac);
    }

    Ok(actual_sample_rate)
}
