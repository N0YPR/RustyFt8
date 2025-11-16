///! Spectrogram and sync correlation computation
///!
///! Computes power spectra and 2D sync correlation matrices for FT8 signals.

use super::fft::fft_real;
use super::{COSTAS_PATTERN, SAMPLE_RATE, NMAX, NSPS, NSTEP, NFFT1, NH1, NHSYM, MAX_LAG};

/// Compute power spectrum for each time step
///
/// Computes FFTs every NSTEP samples (480 samples = 40 ms) to build spectrogram
///
/// # Arguments
/// * `signal` - Input signal (15 seconds at 12 kHz = 180,000 samples)
/// * `spectra` - Output spectra [freq_bin][time_step] (NH1 x NHSYM)
///
/// # Returns
/// Average spectrum across all time steps
pub fn compute_spectra(signal: &[f32], spectra: &mut [[f32; NHSYM]]) -> Result<Vec<f32>, String> {
    if signal.len() < NMAX {
        return Err(format!("Signal too short: {} samples (need {})", signal.len(), NMAX));
    }

    if spectra.len() != NH1 {
        return Err(format!("Spectra buffer wrong size: {} (need {})", spectra.len(), NH1));
    }

    let mut avg_spectrum = vec![0.0f32; NH1];
    let fac = 1.0 / 300.0;

    // Buffers for FFT
    let mut x_real = vec![0.0f32; NFFT1];
    let mut x_imag = vec![0.0f32; NFFT1];

    for j in 0..NHSYM {
        let ia = j * NSTEP;
        let ib = ia + NSPS;

        if ib > signal.len() {
            break;
        }

        // Copy and scale input (real part only - clear imaginary)
        for (i, &sample) in signal[ia..ib].iter().enumerate() {
            x_real[i] = fac * sample;
            x_imag[i] = 0.0; // Clear imaginary part for real input
        }
        // Zero-pad the rest
        for i in NSPS..NFFT1 {
            x_real[i] = 0.0;
            x_imag[i] = 0.0;
        }

        // Perform FFT
        fft_real(&mut x_real, &mut x_imag, NFFT1)?;

        // Compute power spectrum
        for i in 0..NH1 {
            let power = x_real[i] * x_real[i] + x_imag[i] * x_imag[i];
            spectra[i][j] = power;
            avg_spectrum[i] += power;
        }
    }

    Ok(avg_spectrum)
}

/// Compute 2D sync correlation matrix
///
/// Correlates signal against Costas arrays at all frequency/time combinations
///
/// # Arguments
/// * `spectra` - Power spectra [freq_bin][time_step]
/// * `freq_min` - Minimum frequency in Hz
/// * `freq_max` - Maximum frequency in Hz
/// * `sync2d` - Output 2D sync matrix [freq_bin][time_lag]
///
/// # Returns
/// Frequency bin range (ia, ib) that was searched
pub fn compute_sync2d(
    spectra: &[[f32; NHSYM]],
    freq_min: f32,
    freq_max: f32,
    sync2d: &mut Vec<Vec<f32>>,
) -> Result<(usize, usize), String> {
    let df = SAMPLE_RATE / NFFT1 as f32; // 3.125 Hz per bin
    let ia = (freq_min / df) as usize;
    let ib = (freq_max / df).min(NH1 as f32 - 1.0) as usize;

    if ia >= ib {
        return Err(format!("Invalid frequency range: {} - {} Hz", freq_min, freq_max));
    }

    // Allocate sync2d if needed
    if sync2d.len() != NH1 {
        *sync2d = vec![vec![0.0f32; (2 * MAX_LAG + 1) as usize]; NH1];
    }

    let nssy = NSPS / NSTEP; // Steps per symbol = 4
    let nfos = NFFT1 / NSPS;  // Frequency oversampling = 2
    let jstrt = (0.5 / (NSTEP as f32 / SAMPLE_RATE)) as i32; // Start at 0.5s

    // For each frequency bin
    for i in ia..=ib {
        // For each time lag
        for j in -MAX_LAG..=MAX_LAG {
            let mut ta = 0.0; // Costas array 1 (symbols 0-6)
            let mut tb = 0.0; // Costas array 2 (symbols 36-42)
            let mut tc = 0.0; // Costas array 3 (symbols 72-78)
            let mut t0a = 0.0; // Baseline for array 1
            let mut t0b = 0.0; // Baseline for array 2
            let mut t0c = 0.0; // Baseline for array 3

            // Sum over 7 Costas tones
            for n in 0..7 {
                let m = j + jstrt + (nssy as i32) * (n as i32);
                let tone = COSTAS_PATTERN[n] as i32;

                // Costas array 1 (at symbol 0)
                if m >= 0 && (m as usize) < NHSYM {
                    let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
                    if freq_idx < NH1 {
                        ta += spectra[freq_idx][m as usize];
                        // Baseline: sum all 7 frequency bins (not just the Costas tone)
                        for k in 0..7 {
                            let baseline_idx = i + nfos * k;
                            if baseline_idx < NH1 {
                                t0a += spectra[baseline_idx][m as usize];
                            }
                        }
                    }
                }

                // Costas array 2 (at symbol 36)
                let m2 = m + (nssy as i32) * 36;
                if m2 >= 0 && (m2 as usize) < NHSYM {
                    let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
                    if freq_idx < NH1 {
                        tb += spectra[freq_idx][m2 as usize];
                        for k in 0..7 {
                            let baseline_idx = i + nfos * k;
                            if baseline_idx < NH1 {
                                t0b += spectra[baseline_idx][m2 as usize];
                            }
                        }
                    }
                }

                // Costas array 3 (at symbol 72)
                let m3 = m + (nssy as i32) * 72;
                if m3 >= 0 && (m3 as usize) < NHSYM {
                    let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
                    if freq_idx < NH1 {
                        tc += spectra[freq_idx][m3 as usize];
                        for k in 0..7 {
                            let baseline_idx = i + nfos * k;
                            if baseline_idx < NH1 {
                                t0c += spectra[baseline_idx][m3 as usize];
                            }
                        }
                    }
                }
            }

            // Compute sync metric: signal / noise_baseline
            let t = ta + tb + tc;
            let mut t0 = t0a + t0b + t0c;
            t0 = (t0 - t) / 6.0; // Normalize baseline
            let sync_abc = if t0 > 0.0 { t / t0 } else { 0.0 };

            // Also try without first Costas (in case signal starts late)
            let t_bc = tb + tc;
            let mut t0_bc = t0b + t0c;
            t0_bc = (t0_bc - t_bc) / 6.0;
            let sync_bc = if t0_bc > 0.0 { t_bc / t0_bc } else { 0.0 };

            // Take the better of the two metrics
            let sync_idx = (j + MAX_LAG) as usize;
            sync2d[i][sync_idx] = sync_abc.max(sync_bc);
        }
    }

    Ok((ia, ib))
}
