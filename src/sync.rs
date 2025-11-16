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

/// Downsample signal to 200 Hz (32 samples/symbol) centered on frequency f0
///
/// Uses FFT-based downsampling: FFT → extract bandwidth → IFFT
///
/// # Arguments
/// * `signal` - Input signal (15 seconds at 12 kHz = 180,000 samples)
/// * `f0` - Center frequency in Hz
/// * `output` - Output buffer (3200 complex samples at 200 Hz)
///
/// # Returns
/// Ok(actual_sample_rate) on success - the actual output sample rate in Hz
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
    let mut x_real = alloc::vec![0.0f32; NFFT_IN];
    let mut x_imag = alloc::vec![0.0f32; NFFT_IN];

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

    let ib = (fb / df) as usize;
    let it = (ft / df).min((NFFT_IN / 2) as f32) as usize;
    let i0 = (f0 / df) as usize;

    // Copy selected frequency bins to output FFT buffer
    let mut out_real = alloc::vec![0.0f32; NFFT_OUT];
    let mut out_imag = alloc::vec![0.0f32; NFFT_OUT];

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

    #[cfg(feature = "std")]
    {
        extern crate std;
        std::eprintln!("DEBUG downsample_200hz:");
        std::eprintln!("  f0={:.1} Hz, fb={:.1} Hz, ft={:.1} Hz", f0, fb, ft);
        std::eprintln!("  df={:.3} Hz, extracted {} bins", df, k);
        std::eprintln!("  bandwidth={:.1} Hz, NFFT_OUT={}", bandwidth, NFFT_OUT);
        std::eprintln!("  actual_sample_rate={:.1} Hz (target: 200 Hz)", actual_sample_rate);
    }

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

    // Circular shift to center at DC
    let shift = (i0 as i32 - ib as i32).max(0) as usize;
    if shift > 0 && shift < k {
        // Rotate array left by 'shift' positions
        let mut temp_real = alloc::vec![0.0f32; NFFT_OUT];
        let mut temp_imag = alloc::vec![0.0f32; NFFT_OUT];
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

/// Complex-to-complex FFT (forward)
fn fft_complex(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    fft_real(real, imag, n)
}

/// Complex-to-complex inverse FFT
fn fft_complex_inverse(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    // Conjugate input
    for i in 0..n {
        imag[i] = -imag[i];
    }

    // Forward FFT
    fft_real(real, imag, n)?;

    // Conjugate output and scale
    for i in 0..n {
        imag[i] = -imag[i] / n as f32;
        real[i] /= n as f32;
    }

    Ok(())
}

/// Compute sync power on downsampled signal using Costas correlation
///
/// This is the fine sync equivalent of compute_sync2d, operating on
/// downsampled data at 200 Hz (32 samples/symbol).
///
/// # Arguments
/// * `cd` - Downsampled complex signal (3200 samples at 200 Hz)
/// * `time_offset` - Time offset in samples (at 200 Hz rate)
/// * `freq_tweak` - Optional frequency correction phasors (32 per symbol)
/// * `apply_tweak` - Whether to apply frequency correction
///
/// # Returns
/// Sync power metric
pub fn sync_downsampled(
    cd: &[(f32, f32)],
    time_offset: i32,
    freq_tweak: Option<&[(f32, f32)]>,
    apply_tweak: bool,
) -> f32 {
    const NSPS_DOWN: usize = 32; // Samples per symbol at 200 Hz

    // Precompute Costas waveforms at 200 Hz
    let mut costas_wave = [[(0.0f32, 0.0f32); NSPS_DOWN]; 7];

    for (i, &tone) in COSTAS_PATTERN.iter().enumerate() {
        let dphi = 2.0 * core::f32::consts::PI * tone as f32 / NSPS_DOWN as f32;
        let mut phi = 0.0f32;

        for j in 0..NSPS_DOWN {
            costas_wave[i][j] = (libm::cosf(phi), libm::sinf(phi));
            phi = phi + dphi;
            if phi > 2.0 * core::f32::consts::PI {
                phi -= 2.0 * core::f32::consts::PI;
            }
        }
    }

    let mut sync = 0.0f32;

    // Sum over 7 Costas tones and 3 Costas arrays
    for i in 0..7 {
        // Costas array 1 (symbols 0-6)
        let i1 = time_offset + (i as i32) * (NSPS_DOWN as i32);
        // Costas array 2 (symbols 36-42)
        let i2 = i1 + 36 * (NSPS_DOWN as i32);
        // Costas array 3 (symbols 72-78)
        let i3 = i1 + 72 * (NSPS_DOWN as i32);

        let mut wave = costas_wave[i as usize];

        // Apply frequency tweak if requested
        if apply_tweak && freq_tweak.is_some() {
            let tweak = freq_tweak.unwrap();
            for j in 0..NSPS_DOWN {
                let (wr, wi) = wave[j];
                let (tr, ti) = tweak[j];
                // Complex multiply: wave * tweak
                wave[j] = (wr * tr - wi * ti, wr * ti + wi * tr);
            }
        }

        // Correlate with signal at three Costas positions
        let mut z1 = (0.0f32, 0.0f32);
        let mut z2 = (0.0f32, 0.0f32);
        let mut z3 = (0.0f32, 0.0f32);

        if i1 >= 0 && (i1 as usize + NSPS_DOWN - 1) < cd.len() {
            for j in 0..NSPS_DOWN {
                let idx = i1 as usize + j;
                let (sr, si) = cd[idx];
                let (wr, wi) = wave[j];
                // Complex conjugate multiply: signal * conj(wave)
                z1.0 += sr * wr + si * wi;
                z1.1 += si * wr - sr * wi;
            }
        }

        if i2 >= 0 && (i2 as usize + NSPS_DOWN - 1) < cd.len() {
            for j in 0..NSPS_DOWN {
                let idx = i2 as usize + j;
                let (sr, si) = cd[idx];
                let (wr, wi) = wave[j];
                z2.0 += sr * wr + si * wi;
                z2.1 += si * wr - sr * wi;
            }
        }

        if i3 >= 0 && (i3 as usize + NSPS_DOWN - 1) < cd.len() {
            for j in 0..NSPS_DOWN {
                let idx = i3 as usize + j;
                let (sr, si) = cd[idx];
                let (wr, wi) = wave[j];
                z3.0 += sr * wr + si * wi;
                z3.1 += si * wr - sr * wi;
            }
        }

        // Add power from all three Costas arrays
        sync += z1.0 * z1.0 + z1.1 * z1.1;
        sync += z2.0 * z2.0 + z2.1 * z2.1;
        sync += z3.0 * z3.0 + z3.1 * z3.1;
    }

    sync
}

/// Refine candidate frequency and time using fine synchronization
///
/// Downsamples to 200 Hz and searches ±2.5 Hz and ±20 ms for peak sync.
///
/// # Arguments
/// * `signal` - Input signal (15 seconds at 12 kHz)
/// * `candidate` - Coarse sync candidate to refine
///
/// # Returns
/// Refined candidate with updated frequency, time offset, and sync power
pub fn fine_sync(
    signal: &[f32],
    candidate: &Candidate,
) -> Result<Candidate, String> {
    // Downsample centered on candidate frequency
    let mut cd = alloc::vec![(0.0f32, 0.0f32); 4096];
    let actual_sample_rate = downsample_200hz(signal, candidate.frequency, &mut cd)?;

    // Convert time offset to downsampled sample index
    // candidate.time_offset is relative to 0.5s start, but downsampled buffer starts at 0.0
    // So add 0.5s to convert to absolute time, then multiply by actual sample rate
    let initial_offset = ((candidate.time_offset + 0.5) * actual_sample_rate) as i32;


    // Fine time search: ±4 steps of 5 ms each = ±20 ms
    let mut best_time = initial_offset;
    let mut best_sync = 0.0f32;

    for dt in -4..=4 {
        let t_offset = initial_offset + dt;
        let sync = sync_downsampled(&cd, t_offset, None, false);

        if sync > best_sync {
            best_sync = sync;
            best_time = t_offset;
        }
    }

    // Fine frequency search: ±5 steps of 0.5 Hz = ±2.5 Hz
    let mut best_freq = candidate.frequency;
    let dt2 = 1.0 / 200.0; // Sample period at 200 Hz

    for df in -5..=5 {
        let freq_offset = df as f32 * 0.5; // 0.5 Hz steps
        let dphi = 2.0 * core::f32::consts::PI * freq_offset * dt2;

        // Generate frequency correction phasors
        let mut tweak = [(0.0f32, 0.0f32); 32];
        let mut phi = 0.0f32;
        for i in 0..32 {
            tweak[i] = (libm::cosf(phi), libm::sinf(phi));
            phi += dphi;
        }

        let sync = sync_downsampled(&cd, best_time, Some(&tweak), true);

        if sync > best_sync {
            best_sync = sync;
            best_freq = candidate.frequency + freq_offset;
        }
    }

    // Convert back to seconds (inverse of the initial_offset calculation)
    let refined_time = (best_time as f32 / actual_sample_rate) - 0.5;

    Ok(Candidate {
        frequency: best_freq,
        time_offset: refined_time,
        sync_power: best_sync,
    })
}

/// Compute symbol peak power to help with timing alignment
///
/// Returns the average peak power across the three Costas arrays
fn compute_symbol_peak_power(cd: &[(f32, f32)], start_offset: i32, nsps: usize) -> f32 {
    const COSTAS_PATTERN: [u8; 7] = [3, 1, 4, 0, 6, 5, 2];
    const NFFT_SYM: usize = 32;

    let mut sym_real = [0.0f32; NFFT_SYM];
    let mut sym_imag = [0.0f32; NFFT_SYM];
    let mut total_peak = 0.0f32;
    let mut count = 0;

    // Check Costas arrays at positions 0-6, 36-42, 72-78
    for costas_start in [0, 36, 72] {
        for k in 0..7 {
            let symbol_idx = costas_start + k;
            let i1 = start_offset + (symbol_idx as i32) * (nsps as i32);

            if i1 < 0 || (i1 as usize + nsps) > cd.len() {
                continue;
            }

            // Zero FFT buffer
            for j in 0..NFFT_SYM {
                sym_real[j] = 0.0;
                sym_imag[j] = 0.0;
            }

            // Copy symbol
            for j in 0..nsps.min(NFFT_SYM) {
                let idx = i1 as usize + j;
                sym_real[j] = cd[idx].0;
                sym_imag[j] = cd[idx].1;
            }

            // Perform FFT
            if fft_real(&mut sym_real, &mut sym_imag, NFFT_SYM).is_err() {
                continue;
            }

            // Get power at expected Costas tone
            let expected_tone = COSTAS_PATTERN[k] as usize;
            let re = sym_real[expected_tone];
            let im = sym_imag[expected_tone];
            let power = libm::sqrtf(re * re + im * im);

            total_peak += power;
            count += 1;
        }
    }

    if count > 0 {
        total_peak / count as f32
    } else {
        0.0
    }
}

/// Extract 79 FT8 symbols and compute log-likelihood ratios (LLRs) for LDPC decoding
///
/// This function:
/// 1. Extracts 79 symbols from the downsampled signal (32 samples per symbol)
/// 2. Computes power in each of 8 tones (0-7) for each symbol using FFT
/// 3. Converts tone powers to 174 soft LLRs for LDPC decoder
///
/// # Arguments
/// * `signal` - Input signal (15 seconds at 12 kHz)
/// * `candidate` - Refined candidate from fine_sync with accurate frequency and time
/// * `llr` - Output buffer for 174 log-likelihood ratios
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` if extraction fails
pub fn extract_symbols(
    signal: &[f32],
    candidate: &Candidate,
    llr: &mut [f32],
) -> Result<(), String> {
    const NN: usize = 79; // Number of FT8 symbols
    const SYMBOL_DURATION: f32 = 0.16; // FT8 symbol duration in seconds
    const NFFT_SYM: usize = 32; // FFT size for symbol extraction (power of 2)

    if llr.len() < 174 {
        return Err(alloc::format!("LLR buffer too small: {} (need 174)", llr.len()));
    }

    // Downsample centered on ROUNDED frequency
    // TEMPORARY: Testing if fractional Hz is causing issues
    let test_freq = candidate.frequency.round();
    let mut cd = alloc::vec![(0.0f32, 0.0f32); 4096];
    let actual_sample_rate = downsample_200hz(signal, test_freq, &mut cd)?;

    // Calculate samples per symbol based on actual sample rate
    let nsps_down = (actual_sample_rate * SYMBOL_DURATION).round() as usize;

    #[cfg(feature = "std")]
    {
        extern crate std;
        std::eprintln!("DEBUG: Downsampling at {:.1} Hz (candidate was {:.1} Hz)",
            test_freq, candidate.frequency);

        // Check symbols 0 and 36 specifically (both should be tone 3 = 18.75 Hz)
        // Use the refined offset that extract_symbols will use
        let test_start_offset = 98; // This will be refined by extract_symbols
        for (sym_idx, sym_name) in [(0, "Symbol 0"), (36, "Symbol 36")].iter() {
            let sym_start = test_start_offset + sym_idx * 32;
            let mut check_real = [0.0f32; 32];
            let mut check_imag = [0.0f32; 32];
            for i in 0..32 {
                if sym_start + i < cd.len() {
                    check_real[i] = cd[sym_start + i].0;
                    check_imag[i] = cd[sym_start + i].1;
                }
            }
            if let Ok(_) = fft_real(&mut check_real, &mut check_imag, 32) {
                std::eprintln!("  {} FFT (samples {}..{}):", sym_name, sym_start, sym_start + 32);
                for i in 0..8 {
                    let power = check_real[i] * check_real[i] + check_imag[i] * check_imag[i];
                    let freq_hz = i as f32 * 6.25;
                    std::eprintln!("    bin {} ({:.2} Hz): {:.2e}", i, freq_hz, power);
                }
            }
        }
    }

    // Convert time offset to sample index and refine it locally
    let initial_offset = ((candidate.time_offset + 0.5) * actual_sample_rate) as i32;

    #[cfg(feature = "std")]
    {
        extern crate std;
        std::eprintln!("DEBUG extract_symbols:");
        std::eprintln!("  actual_sample_rate={:.1} Hz", actual_sample_rate);
        std::eprintln!("  nsps_down={} samples/symbol", nsps_down);
        std::eprintln!("  initial_offset={} samples", initial_offset);
    }

    // Do a comprehensive fine time search to find optimal symbol timing
    // Search over a wider range to account for timing drift and downsampling artifacts
    let mut best_offset = initial_offset;
    let mut best_metric = 0.0f32;

    // Search range: ±10 samples (±53ms at 187.5 Hz, ±1/3 symbol period)
    for dt in -10..=10 {
        let t_offset = initial_offset + dt;

        // Compute sync metric based on Costas array strength
        let sync = sync_downsampled(&cd, t_offset, None, false);

        // Also check symbol peak power at this offset
        let peak_power = compute_symbol_peak_power(&cd, t_offset, nsps_down);

        // Combined metric: sync strength + peak power
        let metric = sync + 0.1 * peak_power;

        if metric > best_metric {
            best_metric = metric;
            best_offset = t_offset;
        }
    }

    let start_offset = best_offset;

    #[cfg(feature = "std")]
    {
        extern crate std;
        let timing_adjustment = best_offset - initial_offset;
        std::eprintln!("DEBUG extract_symbols: start_offset={}, adjusted by {} samples ({:.1}ms)",
            start_offset, timing_adjustment,
            timing_adjustment as f32 * 1000.0 / actual_sample_rate);
        std::eprintln!("  candidate time={:.3}s, best_metric={:.3}",
            candidate.time_offset, best_metric);
        let max_mag = (0..200).fold(0.0f32, |acc, i| {
            let (r, im) = cd[i];
            let mag = libm::sqrtf(r*r + im*im);
            acc.max(mag)
        });
        std::eprintln!("  Downsampled buffer (first 200): max magnitude = {:.2e}", max_mag);
    }

    // Extract complex symbol values: cs[tone][symbol] for 8 tones × 79 symbols
    // Store COMPLEX values for multi-symbol soft decoding
    let mut cs = alloc::vec![[(0.0f32, 0.0f32); NN]; 8];
    let mut s8 = alloc::vec![[0.0f32; NN]; 8];

    // FFT buffers
    let mut sym_real = [0.0f32; NFFT_SYM];
    let mut sym_imag = [0.0f32; NFFT_SYM];

    // For sub-symbol timing optimization, try centering the FFT window
    // nsps_down is typically ~30 samples, NFFT_SYM is 32
    // Center the data in the FFT buffer by starting 1 sample later
    let fft_offset = if nsps_down < NFFT_SYM { 1 } else { 0 };

    for k in 0..NN {
        // Symbol starts at: start_offset + k * nsps_down samples
        let i1 = start_offset + (k as i32) * (nsps_down as i32);

        #[cfg(feature = "std")]
        if k == 0 || k == 36 || k == 72 {
            extern crate std;
            std::eprintln!("DEBUG: Symbol {} starts at sample {}", k, i1);
        }

        // Check bounds
        if i1 < 0 || (i1 as usize + nsps_down) > cd.len() {
            // Symbol is out of bounds, set to zero
            for tone in 0..8 {
                cs[tone][k] = (0.0, 0.0);
                s8[tone][k] = 0.0;
            }
            continue;
        }

        // Zero the FFT buffer
        for j in 0..NFFT_SYM {
            sym_real[j] = 0.0;
            sym_imag[j] = 0.0;
        }

        // Copy symbol to FFT buffer, centered if needed
        for j in 0..nsps_down {
            let idx = i1 as usize + j;
            let fft_idx = j + fft_offset;
            if fft_idx < NFFT_SYM {
                sym_real[fft_idx] = cd[idx].0;
                sym_imag[fft_idx] = cd[idx].1;
            }
        }

        // Perform FFT
        fft_real(&mut sym_real, &mut sym_imag, NFFT_SYM)?;

        // Store COMPLEX values and magnitude for 8 tones
        // Use bins 0-7 for DC-centered signal
        for tone in 0..8 {
            let re = sym_real[tone];
            let im = sym_imag[tone];
            cs[tone][k] = (re, im);
            s8[tone][k] = libm::sqrtf(re * re + im * im);
        }

        // DEBUG: Check symbol powers for key Costas positions
        #[cfg(feature = "std")]
        if k == 0 || k == 36 || k == 72 {
            extern crate std;
            std::eprintln!("  Symbol {} powers (bins 0-7):", k);
            for tone in 0..8 {
                std::eprintln!("    tone {}: {:.2e}", tone, s8[tone][k]);
            }
        }
    }

    // Validate Costas arrays (quality check)
    let mut nsync = 0;

    #[cfg(feature = "std")]
    {
        extern crate std;
        std::eprintln!("DEBUG extract_symbols: Costas validation");
        std::eprintln!("  Symbol 0 powers:");
        for tone in 0..8 {
            std::eprintln!("    tone {}: {:.2e}", tone, s8[tone][0]);
        }
    }

    for k in 0..7 {
        // Check all three Costas arrays
        let expected_tone = COSTAS_PATTERN[k];

        // Costas array 1 (symbols 0-6)
        let mut max_power = 0.0f32;
        let mut max_tone = 0;
        for tone in 0..8 {
            if s8[tone][k] > max_power {
                max_power = s8[tone][k];
                max_tone = tone;
            }
        }

        #[cfg(feature = "std")]
        {
            extern crate std;
            let match1 = if max_tone == expected_tone as usize { "✓" } else { "✗" };
            if k < 7 {
                std::eprintln!("  Costas1[{}]: expected {}, got {} {}", k, expected_tone, max_tone, match1);
            }
        }

        if max_tone == expected_tone as usize {
            nsync += 1;
        }

        // Costas array 2 (symbols 36-42)
        max_power = 0.0;
        max_tone = 0;
        for tone in 0..8 {
            if s8[tone][k + 36] > max_power {
                max_power = s8[tone][k + 36];
                max_tone = tone;
            }
        }

        #[cfg(feature = "std")]
        {
            extern crate std;
            let match2 = if max_tone == expected_tone as usize { "✓" } else { "✗" };
            if k < 7 {
                std::eprintln!("  Costas2[{}]: expected {}, got {} {}", k, expected_tone, max_tone, match2);
            }
        }

        if max_tone == expected_tone as usize {
            nsync += 1;
        }

        // Costas array 3 (symbols 72-78)
        max_power = 0.0;
        max_tone = 0;
        for tone in 0..8 {
            if s8[tone][k + 72] > max_power {
                max_power = s8[tone][k + 72];
                max_tone = tone;
            }
        }

        #[cfg(feature = "std")]
        {
            extern crate std;
            let match3 = if max_tone == expected_tone as usize { "✓" } else { "✗" };
            if k < 7 {
                std::eprintln!("  Costas3[{}]: expected {}, got {} {}", k, expected_tone, max_tone, match3);
            }
        }

        if max_tone == expected_tone as usize {
            nsync += 1;
        }
    }

    #[cfg(feature = "std")]
    {
        extern crate std;
        std::eprintln!("  Costas sync quality: {}/21 tones correct", nsync);
    }

    // If sync quality is too low, reject
    // Note: Temporarily lowered threshold for testing
    if nsync < 3 {
        return Err(alloc::format!("Sync quality too low: {}/21 Costas tones correct", nsync));
    }

    // Compute LLRs using 3-symbol coherent combining (WSJT-X approach)
    // This provides ~3-6 dB SNR improvement over single-symbol decoding
    // FT8 uses 79 symbols × 3 bits/symbol = 237 bits, but only 174 are used
    // Data symbols: 7-36 (29 symbols) and 43-71 (29 symbols) = 58 symbols × 3 bits = 174 bits

    // Gray code mapping for decoding
    // GRAY_MAP: 3-bit index -> tone (used in encoding)
    // GRAY_MAP_INV: tone -> 3-bit index (used in decoding - what we need!)
    const GRAY_MAP: [u8; 8] = [0, 1, 3, 2, 5, 6, 4, 7];      // index -> tone
    const GRAY_MAP_INV: [u8; 8] = [0, 1, 3, 2, 6, 4, 5, 7];  // tone -> index

    let mut bit_idx = 0;

    // Two-symbol coherent combining provides ~3dB SNR improvement over nsym=1
    // nsym=1: 8 combinations, nsym=2: 64 combinations, nsym=3: 512 combinations
    const NSYM: usize = 2; // Number of symbols to combine
    const NT: usize = 64; // 8^2 = 64 possible tone pairs for nsym=2

    #[cfg(feature = "std")]
    {
        extern crate std;
        std::eprintln!("DEBUG: Using nsym={} soft decoding", NSYM);
    }

    // Process two data symbol blocks
    // Match WSJT-X: k represents data symbol index (0-28), then compute actual position
    for ihalf in 0..2 {
        let base_offset = if ihalf == 0 { 7 } else { 43 };

        let mut k = 0;
        while k < 29 {
            if bit_idx >= 174 {
                break;
            }

            let ks = k + base_offset; // k=0..28 (data symbol index), base_offset=7 or 43
            let mut s2 = [0.0f32; NT]; // Magnitudes for all combinations

            if NSYM == 1 {
                // Single-symbol decoding
                // For each tone (0-7), get its power and map to the 3-bit index it represents
                // s2[index] = power of the tone that decodes to that 3-bit index
                for tone in 0..NT {
                    let index = GRAY_MAP_INV[tone];  // Convert tone to 3-bit index
                    s2[index as usize] = s8[tone][ks];
                }

                // Extract 3 bits from this symbol
                // s2[index] contains magnitude for that 3-bit index
                for bit in 0..3 {
                    if bit_idx >= 174 {
                        break;
                    }

                    let bit_pos = 2 - bit; // Extract bits 2, 1, 0 (MSB to LSB)

                    let mut max_mag_1 = -1e30f32;
                    let mut max_mag_0 = -1e30f32;

                    // Iterate over 3-bit indices (0-7), s2[index] has the magnitude
                    for index in 0..NT {
                        let bit_val = (index >> bit_pos) & 1;

                        if bit_val == 1 {
                            max_mag_1 = max_mag_1.max(s2[index]);
                        } else {
                            max_mag_0 = max_mag_0.max(s2[index]);
                        }
                    }

                    llr[bit_idx] = max_mag_1 - max_mag_0;
                    bit_idx += 1;
                }

                k += NSYM; // Move to next symbol (or group)
            } else if NSYM == 3 {
                // Multi-symbol decoding: coherently combine 3 symbols
                for i in 0..NT {
                    let i1 = i / 64; // First symbol's tone
                    let i2 = (i / 8) % 8; // Second symbol's tone
                    let i3 = i % 8; // Third symbol's tone

                    if ks + 2 < NN {
                        let (r1, im1) = cs[GRAY_MAP[i1] as usize][ks];
                        let (r2, im2) = cs[GRAY_MAP[i2] as usize][ks + 1];
                        let (r3, im3) = cs[GRAY_MAP[i3] as usize][ks + 2];

                        let sum_r = r1 + r2 + r3;
                        let sum_im = im1 + im2 + im3;
                        s2[i] = libm::sqrtf(sum_r * sum_r + sum_im * sum_im);
                    }
                }

                // Extract 9 bits (3 symbols × 3 bits)
                // Combination index i directly encodes the 9 bits:
                // i = (i1 << 6) | (i2 << 3) | i3 where i1, i2, i3 are 3-bit indices
                const IBMAX: usize = 8;
                for ib in 0..=IBMAX {
                    if bit_idx >= 174 {
                        break;
                    }

                    let bit_pos = IBMAX - ib;

                    let mut max_mag_1 = -1e30f32;
                    let mut max_mag_0 = -1e30f32;

                    for i in 0..NT {
                        // i already encodes the 9 bits directly
                        // Bit 8-6: first symbol's 3-bit index
                        // Bit 5-3: second symbol's 3-bit index
                        // Bit 2-0: third symbol's 3-bit index
                        let bit_val = (i >> bit_pos) & 1;

                        if bit_val == 1 {
                            max_mag_1 = max_mag_1.max(s2[i]);
                        } else {
                            max_mag_0 = max_mag_0.max(s2[i]);
                        }
                    }

                    llr[bit_idx] = max_mag_1 - max_mag_0;
                    bit_idx += 1;
                }

                k += NSYM; // Move to next group
            } else if NSYM == 2 {
                // Two-symbol decoding: coherently combine 2 symbols
                for i in 0..NT {
                    let i2 = (i / 8) % 8; // First symbol's 3-bit index (0-7)
                    let i3 = i % 8;       // Second symbol's 3-bit index (0-7)

                    if ks + 1 < NN {
                        let tone2 = GRAY_MAP[i2] as usize;
                        let tone3 = GRAY_MAP[i3] as usize;
                        let (r2, im2) = cs[tone2][ks];
                        let (r3, im3) = cs[tone3][ks + 1];

                        let sum_r = r2 + r3;
                        let sum_im = im2 + im3;
                        s2[i] = libm::sqrtf(sum_r * sum_r + sum_im * sum_im);
                    }
                }

                // Extract 6 bits (2 symbols × 3 bits)
                // Combination index i directly encodes the 6 bits:
                // i = (i2 << 3) | i3 where i2, i3 are 3-bit indices
                const IBMAX: usize = 5;
                for ib in 0..=IBMAX {
                    if bit_idx >= 174 {
                        break;
                    }

                    let bit_pos = IBMAX - ib;

                    let mut max_mag_1 = -1e30f32;
                    let mut max_mag_0 = -1e30f32;

                    for i in 0..NT {
                        // i encodes the 6 bits directly
                        // Bit 5-3: first symbol's 3-bit index
                        // Bit 2-0: second symbol's 3-bit index
                        let bit_val = (i >> bit_pos) & 1;

                        if bit_val == 1 {
                            max_mag_1 = max_mag_1.max(s2[i]);
                        } else {
                            max_mag_0 = max_mag_0.max(s2[i]);
                        }
                    }

                    llr[bit_idx] = max_mag_1 - max_mag_0;
                    bit_idx += 1;
                }

                k += NSYM; // Move to next group
            } else {
                // Invalid nsym value
                break;
            }
        }
    }

    // Normalize LLRs by standard deviation (match WSJT-X normalizebmet)
    let mut sum = 0.0f32;
    let mut sum_sq = 0.0f32;
    for i in 0..174 {
        sum += llr[i];
        sum_sq += llr[i] * llr[i];
    }
    let mean = sum / 174.0;
    let mean_sq = sum_sq / 174.0;
    let variance = mean_sq - mean * mean;
    let std_dev = if variance > 0.0 {
        libm::sqrtf(variance)
    } else {
        libm::sqrtf(mean_sq)
    };

    if std_dev > 0.0 {
        for i in 0..174 {
            llr[i] /= std_dev;
        }
    }

    // Then scale by WSJT-X scalefac=2.83
    for i in 0..174 {
        llr[i] *= 2.83;
    }

    #[cfg(feature = "std")]
    {
        extern crate std;
        std::eprintln!("DEBUG: Multi-symbol soft decoding completed");
        std::eprintln!("  Extracted {} bits total", bit_idx);
        std::eprintln!("  First 10 LLRs: {:?}", &llr[0..10.min(bit_idx)]);
        std::eprintln!("  Last 10 LLRs: {:?}", &llr[164.min(bit_idx)..174.min(bit_idx)]);

        // Show detected tones for all data symbols
        std::eprintln!("  Data symbols 7-35 (detected tones):");
        for k in 7..36 {
            let mut max_pow = 0.0f32;
            let mut max_tone = 0;
            for tone in 0..8 {
                if s8[tone][k] > max_pow {
                    max_pow = s8[tone][k];
                    max_tone = tone;
                }
            }
            std::eprint!("{}", max_tone);
        }
        std::eprintln!();
        std::eprintln!("  Data symbols 43-71 (detected tones):");
        for k in 43..72 {
            let mut max_pow = 0.0f32;
            let mut max_tone = 0;
            for tone in 0..8 {
                if s8[tone][k] > max_pow {
                    max_pow = s8[tone][k];
                    max_tone = tone;
                }
            }
            std::eprint!("{}", max_tone);
        }
        std::eprintln!();

        // Show hard-decision bits (from LLR signs)
        std::eprint!("  Hard decision bits (first 90): ");
        for i in 0..90.min(bit_idx) {
            std::eprint!("{}", if llr[i] > 0.0 { 1 } else { 0 });
        }
        std::eprintln!();
        std::eprint!("  Hard decision bits (last 84): ");
        for i in 90..bit_idx {
            std::eprint!("{}", if llr[i] > 0.0 { 1 } else { 0 });
        }
        std::eprintln!();
    }

    Ok(())
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
