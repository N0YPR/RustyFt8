///! Candidate signal detection and ranking
///!
///! Identifies potential FT8 signals from sync correlation data.

use super::{SAMPLE_RATE, NFFT1, NSTEP, MAX_LAG, COARSE_LAG};
use super::spectra::{compute_spectra, compute_sync2d};

/// Candidate signal found during coarse sync
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    /// Center frequency in Hz
    pub frequency: f32,
    /// Time offset in seconds from start of 15s window
    pub time_offset: f32,
    /// Sync quality metric (higher is better)
    pub sync_power: f32,
    /// Baseline noise power at this frequency (linear scale, from average spectrum)
    pub baseline_noise: f32,
}

/// Find candidate signals from sync2d correlation matrix
///
/// Identifies peaks in the 2D sync matrix and ranks them by quality.
///
/// # Arguments
/// * `sync2d` - 2D sync correlation matrix [freq_bin][time_lag]
/// * `ia` - Starting frequency bin index
/// * `ib` - Ending frequency bin index
/// * `sync_min` - Minimum sync power threshold (after normalization)
/// * `max_candidates` - Maximum number of candidates to return
/// * `avg_spectrum` - Average power spectrum (linear scale) for baseline noise lookup
///
/// # Returns
/// Vector of candidates sorted by sync power (descending)
pub fn find_candidates(
    sync2d: &[Vec<f32>],
    ia: usize,
    ib: usize,
    sync_min: f32,
    max_candidates: usize,
    avg_spectrum: &[f32],
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

        // Look up baseline noise at this frequency
        let baseline_noise = if i < avg_spectrum.len() {
            avg_spectrum[i].max(1e-30) // Ensure non-zero
        } else {
            1e-30
        };

        // Add both peaks (will filter by threshold after normalization)
        if best_sync > 0.0 {
            candidates.push(Candidate {
                frequency: i as f32 * df,
                time_offset: (best_lag as f32 - 0.5) * tstep,
                sync_power: best_sync,
                baseline_noise,
            });
        }

        if best_lag2 != best_lag && best_sync2 > 0.0 {
            candidates.push(Candidate {
                frequency: i as f32 * df,
                time_offset: (best_lag2 as f32 - 0.5) * tstep,
                sync_power: best_sync2,
                baseline_noise,
            });
        }
    }

    // Normalize sync powers to relative scale
    if !candidates.is_empty() {
        // Find 40th percentile for baseline
        let mut sync_values: Vec<f32> = candidates.iter().map(|c| c.sync_power).collect();
        sync_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        let percentile_idx = (sync_values.len() as f32 * 0.4) as usize;
        let baseline = sync_values[percentile_idx];

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
    let mut spectra = vec![[0.0f32; super::NHSYM]; super::NH1];

    // Compute power spectra and get average spectrum
    let avg_spectrum = compute_spectra(signal, &mut spectra)?;

    // Compute baseline noise spectrum using WSJT-X polynomial fitting algorithm
    let baseline_db = super::compute_baseline(&avg_spectrum, freq_min, freq_max);

    // Convert baseline from dB to linear scale using WSJT-X formula:
    // xbase = 10^(0.1*(sbase[bin]-40.0))
    let mut baseline_linear = vec![0.0f32; baseline_db.len()];
    for i in 0..baseline_db.len() {
        baseline_linear[i] = 10.0f32.powf(0.1 * (baseline_db[i] - 40.0));
    }

    // Compute 2D sync correlation
    let mut sync2d = Vec::new();
    let (ia, ib) = compute_sync2d(&spectra, freq_min, freq_max, &mut sync2d)?;

    // Find and rank candidates (pass baseline_linear for noise estimation)
    let candidates = find_candidates(&sync2d, ia, ib, sync_min, max_candidates, &baseline_linear);

    Ok(candidates)
}
