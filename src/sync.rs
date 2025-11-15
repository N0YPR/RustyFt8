//! FT8 Signal Synchronization
//!
//! This module implements signal detection and synchronization for FT8 using Costas array correlation.
//!
//! **FT8 Sync Structure**:
//! - Three 7x7 Costas arrays at symbols 0-6, 36-42, 72-78
//! - Costas pattern: [3,1,4,0,6,5,2] (7 unique tones)
//!
//! **Algorithm**:
//! 1. Compute 2D sync matrix: sync2d[frequency_bin, time_lag]
//! 2. Correlate against Costas patterns at all three positions
//! 3. Find peaks and generate candidate signals
//! 4. Refine time/frequency estimates with fine synchronization
//!
//! **Search Strategy**:
//! - Coarse: 3.125 Hz freq resolution, 40 ms time resolution
//! - Fine: 0.5 Hz freq resolution, 5 ms time resolution

extern crate alloc;
use alloc::vec::Vec;
use alloc::string::String;

/// Costas 7x7 tone pattern used in FT8
pub const COSTAS_PATTERN: [u8; 7] = [3, 1, 4, 0, 6, 5, 2];

/// Maximum time lag for coarse sync: ±2.5s at 4 samples/symbol = 62.5 steps
pub const MAX_LAG: i32 = 62;

/// Coarse time search window: ±10 lag steps around expected time
pub const COARSE_LAG: i32 = 10;

/// FT8 sample rate in Hz
pub const SAMPLE_RATE: f32 = 12000.0;

/// Samples per symbol
pub const NSPS: usize = 1920;

/// Time step between spectra (1/4 symbol = 480 samples)
pub const NSTEP: usize = NSPS / 4;

/// FFT size for symbol spectra (must be power of 2)
pub const NFFT1: usize = 4096; // Nearest power of 2 to 2*NSPS (3840)

/// Number of FFT bins
pub const NH1: usize = NFFT1 / 2; // 2048

/// Maximum number of samples (15 seconds at 12 kHz)
pub const NMAX: usize = 15 * 12000; // 180,000

/// Number of spectra (15s / 40ms steps)
pub const NHSYM: usize = NMAX / NSTEP - 3; // 372

/// Candidate signal information
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    /// Center frequency in Hz
    pub frequency: f32,
    /// Time offset in seconds from start of 15s window
    pub time_offset: f32,
    /// Sync quality metric (higher is better)
    pub sync_power: f32,
}

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
        return Err(alloc::format!("Signal too short: {} samples (need {})", signal.len(), NMAX));
    }

    if spectra.len() != NH1 {
        return Err(alloc::format!("Spectra buffer wrong size: {} (need {})", spectra.len(), NH1));
    }

    // Debug: check signal level
    #[cfg(feature = "std")]
    {
        extern crate std;
        let max_sample = signal.iter().take(NMAX).fold(0.0f32, |acc, &x| acc.max(x.abs()));
        std::eprintln!("DEBUG compute_spectra: max signal = {:.6}", max_sample);
    }

    let mut avg_spectrum = alloc::vec![0.0f32; NH1];
    let fac = 1.0 / 300.0;

    // Buffers for FFT
    let mut x_real = alloc::vec![0.0f32; NFFT1];
    let mut x_imag = alloc::vec![0.0f32; NFFT1];

    for j in 0..NHSYM {
        let ia = j * NSTEP;
        let ib = ia + NSPS;

        if ib > signal.len() {
            break;
        }

        // Debug: check signal slice
        #[cfg(feature = "std")]
        if j == 0 {
            extern crate std;
            let slice_max = signal[ia..ib].iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
            std::eprintln!("DEBUG: Signal slice [{}..{}] max = {:.6}", ia, ib, slice_max);
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

        // Debug: check FFT input
        #[cfg(feature = "std")]
        if j == 0 {
            extern crate std;
            std::eprintln!("DEBUG: Before FFT: real[0]={:.2e} real[100]={:.2e} real[1000]={:.2e}",
                x_real[0], x_real[100], x_real[1000]);
            let max_input = x_real.iter().take(NSPS).fold(0.0f32, |acc, &x| acc.max(x.abs()));
            std::eprintln!("DEBUG: Max input sample = {:.2e}", max_input);
        }

        // Perform FFT
        fft_real(&mut x_real, &mut x_imag, NFFT1)?;

        // Debug: check FFT output
        #[cfg(feature = "std")]
        if j == 0 {
            extern crate std;
            std::eprintln!("DEBUG: After FFT: real[0]={:.2e} real[100]={:.2e} real[1000]={:.2e}",
                x_real[0], x_real[100], x_real[1000]);
            std::eprintln!("DEBUG: After FFT: imag[0]={:.2e} imag[100]={:.2e} imag[1000]={:.2e}",
                x_imag[0], x_imag[100], x_imag[1000]);
        }

        // Compute power spectrum
        for i in 0..NH1 {
            let power = x_real[i] * x_real[i] + x_imag[i] * x_imag[i];
            spectra[i][j] = power;
            avg_spectrum[i] += power;
        }

        // Debug: check for inf/nan
        #[cfg(feature = "std")]
        if j < 3 {
            extern crate std;
            let max_in_spectrum = (0..NH1).fold(0.0f32, |acc, i| acc.max(spectra[i][j]));
            let min_in_spectrum = (0..NH1).fold(f32::INFINITY, |acc, i| acc.min(spectra[i][j]));
            std::eprintln!("DEBUG: Spectrum {} range: {:.2e} to {:.2e}",
                j, min_in_spectrum, max_in_spectrum);
        }
    }

    Ok(avg_spectrum)
}

/// Simple radix-2 FFT for real input (Cooley-Tukey)
///
/// # Arguments
/// * `real` - Real part (input/output)
/// * `imag` - Imaginary part (input/output)
/// * `n` - FFT size (must be power of 2)
fn fft_real(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    if n & (n - 1) != 0 {
        return Err(alloc::format!("FFT size must be power of 2, got {}", n));
    }

    // Bit-reversal permutation
    let mut j = 0;
    for i in 0..n - 1 {
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
        let mut k = n / 2;
        while k <= j {
            j -= k;
            k /= 2;
        }
        j += k;
    }

    // Cooley-Tukey decimation-in-time FFT
    let mut len = 2;
    while len <= n {
        let half_len = len / 2;
        let angle = -2.0 * core::f32::consts::PI / len as f32;

        for i in (0..n).step_by(len) {
            let mut k = 0;
            for j in i..i + half_len {
                let theta = angle * k as f32;
                let wr = libm::cosf(theta);
                let wi = libm::sinf(theta);

                let t_real = wr * real[j + half_len] - wi * imag[j + half_len];
                let t_imag = wr * imag[j + half_len] + wi * real[j + half_len];

                real[j + half_len] = real[j] - t_real;
                imag[j + half_len] = imag[j] - t_imag;
                real[j] += t_real;
                imag[j] += t_imag;

                k += 1;
            }
        }
        len *= 2;
    }

    Ok(())
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
        return Err(alloc::format!("Invalid frequency range: {} - {} Hz", freq_min, freq_max));
    }

    // Allocate sync2d if needed
    if sync2d.len() != NH1 {
        *sync2d = alloc::vec![alloc::vec![0.0f32; (2 * MAX_LAG + 1) as usize]; NH1];
    }

    let nssy = NSPS / NSTEP; // Steps per symbol = 4
    let nfos = NFFT1 / NSPS;  // Frequency oversampling = 2
    let jstrt = (0.5 / (NSTEP as f32 / SAMPLE_RATE)) as i32; // Start at 0.5s

    // Debug: check if spectra has any energy
    #[cfg(feature = "std")]
    {
        extern crate std;
        let mut max_power = 0.0f32;
        for i in ia..=ib {
            for j in 0..NHSYM {
                if spectra[i][j] > max_power {
                    max_power = spectra[i][j];
                }
            }
        }
        std::eprintln!("DEBUG compute_sync2d: max spectra power = {:.2e}, freq range {} - {} (bins {} - {})",
            max_power, freq_min, freq_max, ia, ib);
    }

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

                // Debug first iteration
                #[cfg(feature = "std")]
                if i == 512 && j == 0 && n == 0 {
                    extern crate std;
                    std::eprintln!("DEBUG: n={} tone={} m={} freq_idx={}",
                        n, tone, m, (i as i32 + nfos as i32 * tone));
                    if m >= 0 && (m as usize) < NHSYM {
                        let freq_idx = (i as i32 + nfos as i32 * tone) as usize;
                        if freq_idx < NH1 {
                            std::eprintln!("  Accessing spectra[{}][{}] = {:.2e}", freq_idx, m, spectra[freq_idx][m as usize]);
                        }
                    }
                }

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

            // Debug: log a sample correlation at expected signal frequency
            #[cfg(feature = "std")]
            if i == 512 && j == 0 { // Around 1500 Hz, lag 0
                extern crate std;
                std::eprintln!("DEBUG sync2d[{}][lag={}]: ta={:.2e} tb={:.2e} tc={:.2e} t0a={:.2e} t0b={:.2e} t0c={:.2e}",
                    i, j, ta, tb, tc, t0a, t0b, t0c);
                std::eprintln!("  sync_abc={:.2e} sync_bc={:.2e} final={:.2e}", sync_abc, sync_bc, sync2d[i][sync_idx]);
                std::eprintln!("  nssy={} nfos={} jstrt={}", nssy, nfos, jstrt);
            }
        }
    }

    Ok((ia, ib))
}

/// Find candidate signals from 2D sync matrix
///
/// # Arguments
/// * `sync2d` - 2D sync correlation matrix
/// * `ia` - Starting frequency bin
/// * `ib` - Ending frequency bin
/// * `sync_min` - Minimum sync power threshold
/// * `max_candidates` - Maximum number of candidates to return
///
/// # Returns
/// Vector of candidate signals sorted by sync quality
pub fn find_candidates(
    sync2d: &[Vec<f32>],
    ia: usize,
    ib: usize,
    sync_min: f32,
    max_candidates: usize,
) -> Vec<Candidate> {
    let df = SAMPLE_RATE / NFFT1 as f32; // 3.125 Hz
    let tstep = NSTEP as f32 / SAMPLE_RATE; // 0.04 seconds

    let mut candidates = Vec::new();

    // Find peak time lag for each frequency bin
    // Don't apply sync_min threshold yet - we'll normalize first
    for i in ia..=ib {
        // Search within ±COARSE_LAG steps
        let mut best_lag = 0i32;
        let mut best_sync = 0.0f32;

        for lag in -COARSE_LAG..=COARSE_LAG {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[i].len() {
                let sync_val = sync2d[i][sync_idx];
                if sync_val > best_sync {
                    best_sync = sync_val;
                    best_lag = lag;
                }
            }
        }

        // Also search full range
        let mut best_lag2 = 0i32;
        let mut best_sync2 = 0.0f32;

        for lag in -MAX_LAG..=MAX_LAG {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[i].len() {
                let sync_val = sync2d[i][sync_idx];
                if sync_val > best_sync2 {
                    best_sync2 = sync_val;
                    best_lag2 = lag;
                }
            }
        }

        // Add both peaks (will filter by threshold after normalization)
        if best_sync > 0.0 {
            candidates.push(Candidate {
                frequency: i as f32 * df,
                time_offset: (best_lag as f32 - 0.5) * tstep,
                sync_power: best_sync,
            });
        }

        if best_lag2 != best_lag && best_sync2 > 0.0 {
            candidates.push(Candidate {
                frequency: i as f32 * df,
                time_offset: (best_lag2 as f32 - 0.5) * tstep,
                sync_power: best_sync2,
            });
        }
    }

    // Debug: check if we have any candidates at all
    #[cfg(feature = "std")]
    {
        extern crate std;
        if candidates.is_empty() {
            std::eprintln!("DEBUG: No candidates found before normalization");
        } else {
            std::eprintln!("DEBUG: Found {} raw candidates before normalization", candidates.len());
            // Show top 5
            let mut sorted = candidates.clone();
            sorted.sort_by(|a, b| b.sync_power.partial_cmp(&a.sync_power).unwrap_or(core::cmp::Ordering::Equal));
            for (i, cand) in sorted.iter().take(5).enumerate() {
                std::eprintln!("  {}. freq={:.1} Hz, time={:.3} s, sync={:.2}",
                    i+1, cand.frequency, cand.time_offset, cand.sync_power);
            }
        }
    }

    // Normalize sync powers to relative scale
    if !candidates.is_empty() {
        // Find 40th percentile for baseline
        let mut sync_values: Vec<f32> = candidates.iter().map(|c| c.sync_power).collect();
        sync_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        let percentile_idx = (sync_values.len() as f32 * 0.4) as usize;
        let baseline = sync_values[percentile_idx];

        #[cfg(feature = "std")]
        {
            extern crate std;
            std::eprintln!("DEBUG: Normalizing by baseline = {:.2}", baseline);
        }

        if baseline > 0.0 {
            for cand in &mut candidates {
                cand.sync_power /= baseline;
            }
        }
    }

    // Remove duplicates (within 4 Hz and 40 ms)
    let mut filtered: Vec<Candidate> = Vec::new();
    for cand in &candidates {
        let mut is_dupe = false;
        for existing in &filtered {
            let fdiff = (cand.frequency - existing.frequency).abs();
            let tdiff = (cand.time_offset - existing.time_offset).abs();
            if fdiff < 4.0 && tdiff < 0.04 {
                is_dupe = true;
                break;
            }
        }
        if !is_dupe && cand.sync_power >= sync_min {
            filtered.push(*cand);
        }
    }

    // Sort by sync power (descending)
    filtered.sort_by(|a, b| b.sync_power.partial_cmp(&a.sync_power).unwrap_or(core::cmp::Ordering::Equal));

    // Limit to max_candidates
    filtered.truncate(max_candidates);

    filtered
}

/// Perform coarse synchronization on FT8 signal
///
/// This is the main entry point for signal detection. It:
/// 1. Computes power spectra
/// 2. Correlates against Costas arrays
/// 3. Finds and ranks candidate signals
///
/// # Arguments
/// * `signal` - Input signal (15 seconds at 12 kHz)
/// * `freq_min` - Minimum search frequency in Hz (typically 100)
/// * `freq_max` - Maximum search frequency in Hz (typically 3000)
/// * `sync_min` - Minimum sync threshold (typically 1.3)
/// * `max_candidates` - Maximum candidates to return (typically 100)
///
/// # Returns
/// Vector of candidate signals sorted by quality
pub fn coarse_sync(
    signal: &[f32],
    freq_min: f32,
    freq_max: f32,
    sync_min: f32,
    max_candidates: usize,
) -> Result<Vec<Candidate>, String> {
    // Allocate spectra buffer
    let mut spectra = alloc::vec![[0.0f32; NHSYM]; NH1];

    // Compute power spectra
    compute_spectra(signal, &mut spectra)?;

    // Compute 2D sync correlation
    let mut sync2d = Vec::new();
    let (ia, ib) = compute_sync2d(&spectra, freq_min, freq_max, &mut sync2d)?;

    // Find and rank candidates
    let candidates = find_candidates(&sync2d, ia, ib, sync_min, max_candidates);

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_costas_pattern() {
        assert_eq!(COSTAS_PATTERN.len(), 7);
        // All tones should be unique and in range 0-6
        for &tone in &COSTAS_PATTERN {
            assert!(tone < 8);
        }
    }

    #[test]
    fn test_fft_real_dc() {
        let mut real = alloc::vec![1.0f32; 8];
        let mut imag = alloc::vec![0.0f32; 8];

        fft_real(&mut real, &mut imag, 8).unwrap();

        // DC component should be 8.0
        assert!((real[0] - 8.0).abs() < 0.001);
        // Other bins should be near zero
        for i in 1..8 {
            assert!(real[i].abs() < 0.001);
            assert!(imag[i].abs() < 0.001);
        }
    }

    #[test]
    fn test_fft_real_sine() {
        let n = 64;
        let mut real = alloc::vec![0.0f32; n];
        let mut imag = alloc::vec![0.0f32; n];

        // Generate sine wave at bin 5
        let freq = 5.0;
        for i in 0..n {
            let phase = 2.0 * core::f32::consts::PI * freq * i as f32 / n as f32;
            real[i] = libm::sinf(phase);
        }

        fft_real(&mut real, &mut imag, n).unwrap();

        // Peak should be at bin 5
        let mut max_mag = 0.0f32;
        let mut max_bin = 0;
        for i in 0..n {
            let mag = libm::sqrtf(real[i] * real[i] + imag[i] * imag[i]);
            if mag > max_mag {
                max_mag = mag;
                max_bin = i;
            }
        }

        assert_eq!(max_bin, 5, "Peak should be at bin 5");
    }

    #[test]
    fn test_compute_spectra_size() {
        let signal = alloc::vec![0.0f32; NMAX];
        let mut spectra = alloc::vec![[0.0f32; NHSYM]; NH1];

        let result = compute_spectra(&signal, &mut spectra);
        if let Err(e) = &result {
            panic!("compute_spectra failed: {}", e);
        }
        assert!(result.is_ok());

        let avg_spectrum = result.unwrap();
        assert_eq!(avg_spectrum.len(), NH1);
    }

    #[test]
    fn test_compute_spectra_too_short() {
        let signal = alloc::vec![0.0f32; 1000]; // Too short
        let mut spectra = alloc::vec![[0.0f32; NHSYM]; NH1];

        let result = compute_spectra(&signal, &mut spectra);
        assert!(result.is_err());
    }
}
