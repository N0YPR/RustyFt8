///! Coarse synchronization
///!
///! Identifies potential FT8 signals from sync correlation data.

use super::{SAMPLE_RATE, NFFT1, NSTEP, MAX_LAG, COARSE_LAG, Candidate};
use super::spectra::{compute_spectra, compute_sync2d};
use tracing::{debug, info, trace, instrument};



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
#[instrument(skip(signal), fields(signal_len = signal.len()))]
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

/// Find candidate signals from sync2d correlation matrix
///
/// Matches WSJT-X sync8.f90 algorithm exactly:
/// 1. Find peaks for all frequency bins → red[i], jpeak[i], red2[i], jpeak2[i]
/// 2. Find 40th percentile of red values → normalize ALL bins
/// 3. Sort bins by normalized sync power
/// 4. Create candidates from bins with sync >= syncmin
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
#[instrument(skip(sync2d, avg_spectrum), fields(freq_bins = ib - ia + 1))]
fn find_candidates(
    sync2d: &[Vec<f32>],
    ia: usize,
    ib: usize,
    sync_min: f32,
    max_candidates: usize,
    avg_spectrum: &[f32],
) -> Vec<Candidate> {
    let df = SAMPLE_RATE / NFFT1 as f32; // 3.125 Hz
    let tstep = NSTEP as f32 / SAMPLE_RATE; // 0.04 seconds

    // Allocate per-bin arrays (matching WSJT-X sync8.f90 lines 14-20)
    let nbins = ib - ia + 1;
    let mut red = vec![0.0f32; nbins];      // Narrow search sync power
    let mut jpeak = vec![0i32; nbins];      // Narrow search peak lag
    let mut red2 = vec![0.0f32; nbins];     // Wide search sync power
    let mut jpeak2 = vec![0i32; nbins];     // Wide search peak lag

    // Dual peak search strategy (matching WSJT-X sync8.f90 lines 89-97)
    // 1. Narrow search: ±10 steps around zero lag (±0.4s) for typical signals
    // 2. Wide search: ±MAX_LAG steps (±2.48s) for early/late signals
    // NO time penalty - use raw sync power like WSJT-X
    const NARROW_LAG: i32 = 10; // ±0.4s search range for primary peak

    // STEP 1: Find peaks for all frequency bins (WSJT-X sync8.f90 lines 91-98)
    for i in ia..=ib {
        let bin_idx = i - ia;

        // Narrow search: ±10 steps
        let mut peak_narrow = 0i32;
        let mut red_narrow = 0.0f32;
        for lag in -NARROW_LAG..=NARROW_LAG {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[i].len() {
                let sync_val = sync2d[i][sync_idx];
                if sync_val > red_narrow {
                    red_narrow = sync_val;
                    peak_narrow = lag;
                }
            }
        }
        red[bin_idx] = red_narrow;
        jpeak[bin_idx] = peak_narrow;

        // Wide search: ±MAX_LAG steps
        let mut peak_wide = 0i32;
        let mut red_wide = 0.0f32;
        for lag in -MAX_LAG..=MAX_LAG {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[i].len() {
                let sync_val = sync2d[i][sync_idx];
                if sync_val > red_wide {
                    red_wide = sync_val;
                    peak_wide = lag;
                }
            }
        }
        red2[bin_idx] = red_wide;
        jpeak2[bin_idx] = peak_wide;
    }

    // Debug: Show raw sync values at key WSJT-X candidate bins (use RUST_LOG=trace)
    if tracing::enabled!(tracing::Level::TRACE) {
        let debug_freqs = [
            (1490.6, 477, 0.020), // freq, bin, expected_time
            (1493.8, 478, 0.060),
            (1506.2, 482, 0.380),
            (2571.9, 823, 0.300),
            (2534.4, 811, 2.380)
        ];
        for &(freq, expected_bin, expected_time) in &debug_freqs {
            if expected_bin >= ia && expected_bin <= ib {
                let bin_idx = expected_bin - ia;
                let computed_time_narrow = (jpeak[bin_idx] as f32 - 0.5) * tstep;
                let computed_time_wide = (jpeak2[bin_idx] as f32 - 0.5) * tstep;
                trace!(
                    freq = %freq, bin = %expected_bin,
                    red_narrow = %red[bin_idx], jpeak_narrow = %jpeak[bin_idx], time_narrow = %computed_time_narrow,
                    red_wide = %red2[bin_idx], jpeak_wide = %jpeak2[bin_idx], time_wide = %computed_time_wide,
                    expected_time = %expected_time,
                    "sync at WSJT-X bin: narrow vs wide search"
                );
            }
        }
    }

    // STEP 2: Normalize ALL bins using 40th percentile (WSJT-X sync8.f90 lines 100-116)
    // This is the CRITICAL fix - normalize BEFORE filtering, not after!

    // Sort red values to find 40th percentile
    let mut red_sorted = red.clone();
    red_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    // WSJT-X uses nint(0.40*iz) which rounds to nearest integer
    let percentile_idx = (nbins as f32 * 0.4).round() as usize;
    let baseline = red_sorted[percentile_idx].max(1e-30);

    debug!(
        bins_searched = nbins,
        percentile_idx = percentile_idx,
        baseline_40th = %baseline,
        "normalizing all bins by 40th percentile"
    );

    // Debug: Show values around percentile for comparison with WSJT-X
    if tracing::enabled!(tracing::Level::TRACE) {
        trace!(
            idx_minus_5 = %red_sorted[percentile_idx.saturating_sub(5)],
            idx_minus_1 = %red_sorted[percentile_idx.saturating_sub(1)],
            idx_0 = %red_sorted[percentile_idx],
            idx_plus_1 = %red_sorted[(percentile_idx + 1).min(nbins - 1)],
            idx_plus_5 = %red_sorted[(percentile_idx + 5).min(nbins - 1)],
            "values around 40th percentile"
        );
    }

    // Normalize ALL red values by baseline
    for val in &mut red {
        *val /= baseline;
    }

    // Do same for red2
    let mut red2_sorted = red2.clone();
    red2_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
    let baseline2 = red2_sorted[percentile_idx].max(1e-30);
    for val in &mut red2 {
        *val /= baseline2;
    }

    // STEP 3: Create index array sorted by red (descending)
    let mut indices: Vec<usize> = (0..nbins).collect();
    indices.sort_by(|&a, &b| red[b].partial_cmp(&red[a]).unwrap_or(core::cmp::Ordering::Equal));

    // STEP 4: Generate candidates from sorted bins (WSJT-X sync8.f90 lines 117-134)
    let mut candidates = Vec::new();

    for &bin_idx in &indices {
        if candidates.len() >= max_candidates * 2 {
            break; // Pre-limit before deduplication
        }

        let i = ia + bin_idx;  // Actual frequency bin
        let freq = i as f32 * df;  // NO INTERPOLATION - just bin center

        // Look up baseline noise at this frequency
        let baseline_noise = if i < avg_spectrum.len() {
            avg_spectrum[i].max(1e-30)
        } else {
            1e-30
        };

        // Add candidate from narrow search (filter by sync_min later, after we have enough)
        candidates.push(Candidate {
            frequency: freq,
            time_offset: (jpeak[bin_idx] as f32 - 0.5) * tstep,
            sync_power: red[bin_idx],
            baseline_noise,
        });

        // Add second candidate from wide search if different peak
        if jpeak2[bin_idx] != jpeak[bin_idx] {
            candidates.push(Candidate {
                frequency: freq,
                time_offset: (jpeak2[bin_idx] as f32 - 0.5) * tstep,
                sync_power: red2[bin_idx],
                baseline_noise,
            });
        }

        // Stop after we have enough candidates (WSJT-X uses MAXPRECAND=1000)
        if candidates.len() >= 1000 {
            break;
        }
    }

    // Remove duplicates (within 4 Hz and 40 ms)
    let mut filtered: Vec<Candidate> = Vec::new();
    for cand in &candidates {
        let mut is_dupe = false;
        let mut dupe_of: Option<f32> = None;
        for existing in &filtered {
            let fdiff = (cand.frequency - existing.frequency).abs();
            let tdiff = (cand.time_offset - existing.time_offset).abs();
            if fdiff < 4.0 && tdiff < 0.04 {
                is_dupe = true;
                dupe_of = Some(existing.frequency);
                break;
            }
        }

        // Debug: track 2733 Hz candidates through filtering (use RUST_LOG=trace)
        if tracing::enabled!(tracing::Level::TRACE) {
            let is_2733 = (cand.frequency - 2733.0).abs() < 5.0;
            if is_2733 {
                if is_dupe {
                    trace!(
                        freq = %cand.frequency,
                        sync = %cand.sync_power,
                        time = %cand.time_offset,
                        dupe_of = %dupe_of.unwrap(),
                        "2733 Hz candidate FILTERED as duplicate"
                    );
                } else {
                    trace!(
                        freq = %cand.frequency,
                        sync = %cand.sync_power,
                        time = %cand.time_offset,
                        "2733 Hz candidate PASSED"
                    );
                }
            }
        }

        // Apply sync_min filter here (after creating candidates)
        if !is_dupe && cand.sync_power >= sync_min {
            filtered.push(*cand);
        }
    }

    // Sort by sync power (descending)
    filtered.sort_by(|a, b| b.sync_power.partial_cmp(&a.sync_power).unwrap_or(core::cmp::Ordering::Equal));

    // Limit to max_candidates
    filtered.truncate(max_candidates);

    info!(
        total_candidates = filtered.len(),
        max_candidates = max_candidates,
        "coarse sync complete"
    );

    if tracing::enabled!(tracing::Level::DEBUG) && !filtered.is_empty() {
        debug!(
            top_freq = %filtered[0].frequency,
            top_sync = %filtered[0].sync_power,
            top_time = %filtered[0].time_offset,
            "top candidate"
        );
    }

    filtered
}
