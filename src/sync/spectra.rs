///! Spectrogram and sync correlation computation
///!
///! Computes power spectra and 2D sync correlation matrices for FT8 signals.

use super::fft::fft_real;
use super::{COSTAS_PATTERN, SAMPLE_RATE, NMAX, NSPS, NSTEP, NFFT1, NH1, NHSYM, MAX_LAG};
use rayon::prelude::*;

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

    let fac = 1.0 / 300.0;

    // Process FFTs in parallel, collecting results
    let fft_results: Vec<Option<Vec<f32>>> = (0..NHSYM)
        .into_par_iter()
        .map(|j| {
            let ia = j * NSTEP;
            let ib = ia + NSPS;

            if ib > signal.len() {
                return None;
            }

            // Allocate FFT buffers per thread
            let mut x_real = vec![0.0f32; NFFT1];
            let mut x_imag = vec![0.0f32; NFFT1];

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
            if fft_real(&mut x_real, &mut x_imag, NFFT1).is_err() {
                return None;
            }

            // Compute power spectrum for this time step
            let mut power_spectrum = vec![0.0f32; NH1];
            for i in 0..NH1 {
                power_spectrum[i] = x_real[i] * x_real[i] + x_imag[i] * x_imag[i];
            }

            Some(power_spectrum)
        })
        .collect();

    // Combine results into spectra and avg_spectrum
    let mut avg_spectrum = vec![0.0f32; NH1];
    for (j, result) in fft_results.iter().enumerate() {
        if let Some(power_spectrum) = result {
            for i in 0..NH1 {
                spectra[i][j] = power_spectrum[i];
                avg_spectrum[i] += power_spectrum[i];
            }
        }
    }

    Ok(avg_spectrum)
}

/// Compute baseline noise spectrum using WSJT-X algorithm
///
/// Fits a polynomial to the "lower envelope" of the average spectrum.
/// Based on WSJT-X baseline.f90
///
/// # Arguments
/// * `avg_spectrum` - Average power spectrum (linear scale)
/// * `freq_min` - Minimum frequency in Hz
/// * `freq_max` - Maximum frequency in Hz
///
/// # Returns
/// Baseline spectrum in dB
pub fn compute_baseline(avg_spectrum: &[f32], freq_min: f32, freq_max: f32) -> Vec<f32> {
    const NSEG: usize = 10; // Number of segments
    const NPCT: usize = 10; // Percentile for lower envelope (10th percentile)

    let df = SAMPLE_RATE / NFFT1 as f32; // 3.125 Hz
    let ia = (freq_min / df).max(1.0) as usize;
    let ib = ((freq_max / df) as usize).min(NH1 - 1);

    // Convert to dB scale
    let mut s_db = vec![0.0f32; NH1];
    for i in ia..=ib {
        s_db[i] = if avg_spectrum[i] > 1e-30 {
            10.0 * avg_spectrum[i].log10()
        } else {
            -300.0
        };
    }

    // Collect lower envelope points
    let nlen = (ib - ia + 1) / NSEG; // Length of each segment
    let i0 = (ib + ia) / 2; // Midpoint
    let mut x_pts = Vec::new();
    let mut y_pts = Vec::new();

    for n in 0..NSEG {
        let ja = ia + n * nlen;
        let jb = (ja + nlen - 1).min(ib);

        // Find NPCT percentile in this segment
        let mut segment: Vec<f32> = s_db[ja..=jb].to_vec();
        segment.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pct_idx = (segment.len() * NPCT / 100).min(segment.len() - 1);
        let base = segment[pct_idx];

        // Collect all points below this threshold
        for i in ja..=jb {
            if s_db[i] <= base {
                x_pts.push((i as f32 - i0 as f32) as f64);
                y_pts.push(s_db[i] as f64);
            }
        }
    }

    // Fit 5th-order polynomial to lower envelope points
    let coeffs = if x_pts.len() >= 6 {
        polyfit(&x_pts, &y_pts, 5)
    } else if x_pts.len() >= 3 {
        polyfit(&x_pts, &y_pts, 2)
    } else {
        // Fallback to constant baseline
        vec![y_pts.iter().sum::<f64>() / y_pts.len().max(1) as f64]
    };

    // Evaluate polynomial to get baseline
    let mut sbase = vec![0.0f32; NH1];
    for i in ia..=ib {
        let t = (i as f64 - i0 as f64);
        let mut val = 0.0f64;
        for (k, &coeff) in coeffs.iter().enumerate() {
            val += coeff * t.powi(k as i32);
        }
        sbase[i] = (val + 0.65) as f32; // Add 0.65 dB offset like WSJT-X
    }

    sbase
}

/// Fit polynomial using least squares
///
/// Simple polynomial fitting using normal equations
fn polyfit(x: &[f64], y: &[f64], degree: usize) -> Vec<f64> {
    let n = x.len().min(y.len());
    if n == 0 {
        return vec![0.0];
    }

    let degree = degree.min(n - 1);
    let m = degree + 1;

    // Build Vandermonde matrix and solve normal equations
    // X = [1, x, x^2, ..., x^degree]
    // coeffs = (X^T X)^-1 X^T y

    let mut xtx = vec![vec![0.0f64; m]; m];
    let mut xty = vec![0.0f64; m];

    for i in 0..n {
        let xi = x[i];
        let yi = y[i];
        let mut xpow = 1.0;

        for j in 0..m {
            xty[j] += xpow * yi;
            let mut xpow2 = 1.0;
            for k in 0..m {
                xtx[j][k] += xpow * xpow2;
                xpow2 *= xi;
            }
            xpow *= xi;
        }
    }

    // Solve using Gaussian elimination
    gauss_solve(&xtx, &xty)
}

/// Solve linear system using Gaussian elimination with partial pivoting
fn gauss_solve(a: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let n = b.len();
    if n == 0 || a.len() != n {
        return vec![0.0];
    }

    // Create augmented matrix [A|b]
    let mut aug = vec![vec![0.0f64; n + 1]; n];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = a[i][j];
        }
        aug[i][n] = b[i];
    }

    // Forward elimination with partial pivoting
    for k in 0..n {
        // Find pivot
        let mut max_row = k;
        let mut max_val = aug[k][k].abs();
        for i in (k + 1)..n {
            let val = aug[i][k].abs();
            if val > max_val {
                max_val = val;
                max_row = i;
            }
        }

        // Swap rows
        if max_row != k {
            aug.swap(k, max_row);
        }

        // Check for singular matrix
        if aug[k][k].abs() < 1e-12 {
            continue;
        }

        // Eliminate column
        for i in (k + 1)..n {
            let factor = aug[i][k] / aug[k][k];
            for j in k..=n {
                aug[i][j] -= factor * aug[k][j];
            }
        }
    }

    // Back substitution
    let mut x = vec![0.0f64; n];
    for i in (0..n).rev() {
        if aug[i][i].abs() < 1e-12 {
            x[i] = 0.0;
            continue;
        }

        let mut sum = aug[i][n];
        for j in (i + 1)..n {
            sum -= aug[i][j] * x[j];
        }
        x[i] = sum / aug[i][i];
    }

    x
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

    // Process frequency bins in parallel
    let sync_results: Vec<(usize, Vec<f32>)> = (ia..=ib)
        .into_par_iter()
        .map(|i| {
            let mut sync_row = vec![0.0f32; (2 * MAX_LAG + 1) as usize];

            // For each time lag
            for j in -MAX_LAG..=MAX_LAG {
                let mut ta = 0.0; // Costas array 1 (symbols 0-6)
                let mut tb = 0.0; // Costas array 2 (symbols 36-42)
                let mut tc = 0.0; // Costas array 3 (symbols 72-78)
                let mut t0a = 0.0; // Baseline for array 1
                let mut t0b = 0.0; // Baseline for array 2
                let mut t0c = 0.0; // Baseline for array 3

                // Sum over 7 Costas tones
                // CRITICAL: Match WSJT-X sync8.f90 lines 62-74 EXACTLY
                for n in 0..7 {
                    let m = j + jstrt + (nssy as i32) * (n as i32);
                    let tone = COSTAS_PATTERN[n] as i32;

                    // Costas array 1 (symbols 0-6)
                    // WSJT-X: if(m.ge.1.and.m.le.NHSYM) - Fortran 1-indexed
                    // WSJT-X: s(i,m) with m=1 accesses first element (Fortran index 1)
                    // Rust: spectra[i][m-1] with m=1 accesses first element (Rust index 0)
                    if m >= 1 && m <= NHSYM as i32 {
                        let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
                        let time_idx = (m - 1) as usize; // Convert Fortran 1-indexed to Rust 0-indexed
                        // WSJT-X: ta=ta + s(i+nfos*icos7(n),m)
                        ta += spectra[freq_idx][time_idx];

                        // WSJT-X: t0a=t0a + sum(s(i:i+nfos*6:nfos,m))
                        // Baseline: sum all 7 frequency bins at same time
                        for k in 0..7 {
                            let baseline_idx = i + nfos * k;
                            t0a += spectra[baseline_idx][time_idx];
                        }
                    }

                    // Costas array 2 (symbols 36-42)
                    // WSJT-X: NO bounds check (assumes middle Costas in valid range)
                    let m2 = m + (nssy as i32) * 36;
                    if m2 >= 1 && m2 <= NHSYM as i32 {
                        let freq_idx2 = (i as i32 + nfos as i32 * tone) as usize;
                        let time_idx2 = (m2 - 1) as usize; // Convert Fortran 1-indexed to Rust 0-indexed
                        // WSJT-X: tb=tb + s(i+nfos*icos7(n),m+nssy*36)
                        tb += spectra[freq_idx2][time_idx2];

                        // WSJT-X: t0b=t0b + sum(s(i:i+nfos*6:nfos,m+nssy*36))
                        for k in 0..7 {
                            let baseline_idx = i + nfos * k;
                            t0b += spectra[baseline_idx][time_idx2];
                        }
                    }

                    // Costas array 3 (symbols 72-78)
                    // WSJT-X: if(m+nssy*72.le.NHSYM) - Fortran 1-indexed
                    let m3 = m + (nssy as i32) * 72;
                    if m3 >= 1 && m3 <= NHSYM as i32 {
                        let freq_idx3 = (i as i32 + nfos as i32 * tone) as usize;
                        let time_idx3 = (m3 - 1) as usize; // Convert Fortran 1-indexed to Rust 0-indexed
                        // WSJT-X: tc=tc + s(i+nfos*icos7(n),m+nssy*72)
                        tc += spectra[freq_idx3][time_idx3];

                        // WSJT-X: t0c=t0c + sum(s(i:i+nfos*6:nfos,m+nssy*72))
                        for k in 0..7 {
                            let baseline_idx = i + nfos * k;
                            t0c += spectra[baseline_idx][time_idx3];
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
                sync_row[sync_idx] = sync_abc.max(sync_bc);
            }

            (i, sync_row)
        })
        .collect();

    // Copy results into sync2d
    for (i, sync_row) in sync_results {
        sync2d[i] = sync_row;
    }

    Ok((ia, ib))
}
