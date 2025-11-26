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

    let mut candidates = Vec::new();

    // Dual peak search strategy (matching WSJT-X sync8.f90 lines 89-97)
    // 1. Narrow search: ±10 steps around zero lag (±0.4s) for typical signals
    // 2. Wide search: ±MAX_LAG steps (±2.48s) for early/late signals
    // NO time penalty - use raw sync power like WSJT-X
    const NARROW_LAG: i32 = 10; // ±0.4s search range for primary peak

    // Debug: Show sync2d values at key frequencies (use RUST_LOG=trace)
    if tracing::enabled!(tracing::Level::TRACE) {
        // Check frequencies where WSJT-X finds signals: 400, 590, 641, 723, 2157, 2238, 2572, 2695, 2733, 2852
        let debug_freqs = [400.0, 590.0, 641.0, 723.0, 2157.0, 2238.0, 2572.0, 2695.0, 2733.0, 2852.0];
        for &freq in &debug_freqs {
            let bin = (freq / df) as usize;
            if bin >= ia && bin <= ib && bin < sync2d.len() {
                // Find max sync value across all lags for this frequency
                let max_sync = sync2d[bin].iter().enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
                    .map(|(idx, val)| (idx as i32 - MAX_LAG, *val))
                    .unwrap_or((0, 0.0));

                // Also check narrow range ±10
                let narrow_max = (-NARROW_LAG..=NARROW_LAG).map(|lag| {
                    let idx = (lag + MAX_LAG) as usize;
                    if idx < sync2d[bin].len() {
                        (lag, sync2d[bin][idx])
                    } else {
                        (lag, 0.0)
                    }
                }).max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal))
                .unwrap_or((0, 0.0));

                trace!(
                    freq = %freq,
                    bin = %bin,
                    max_sync = %max_sync.1,
                    max_lag = %max_sync.0,
                    max_time = %(max_sync.0 as f32 * tstep),
                    narrow_max = %narrow_max.1,
                    narrow_lag = %narrow_max.0,
                    narrow_time = %(narrow_max.0 as f32 * tstep),
                    "sync2d debug frequency"
                );
            }
        }
    }

    for i in ia..=ib {
        // First search: narrow range ±10 steps
        let mut jpeak = 0i32;
        let mut red = 0.0f32;
        for lag in -NARROW_LAG..=NARROW_LAG {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[i].len() {
                let sync_val = sync2d[i][sync_idx];
                if sync_val > red {
                    red = sync_val;
                    jpeak = lag;
                }
            }
        }

        // Second search: wide range ±MAX_LAG steps
        let mut jpeak2 = 0i32;
        let mut red2 = 0.0f32;
        for lag in -MAX_LAG..=MAX_LAG {
            let sync_idx = (lag + MAX_LAG) as usize;
            if sync_idx < sync2d[i].len() {
                let sync_val = sync2d[i][sync_idx];
                if sync_val > red2 {
                    red2 = sync_val;
                    jpeak2 = lag;
                }
            }
        }

        // Look up baseline noise at this frequency
        let baseline_noise = if i < avg_spectrum.len() {
            avg_spectrum[i].max(1e-30) // Ensure non-zero
        } else {
            1e-30
        };

        // Parabolic interpolation for sub-bin frequency accuracy
        // Refines from 2.93 Hz quantization to ~0.3 Hz accuracy
        // Critical for correct tone extraction (0.2 Hz error causes 20% tone errors!)
        let interpolated_freq = if i > 0 && i < sync2d.len() - 1 {
            // Get sync values at bins i-1, i, i+1 at the peak lag
            let sync_idx = (jpeak + MAX_LAG) as usize;
            if sync_idx < sync2d[i-1].len() && sync_idx < sync2d[i].len() && sync_idx < sync2d[i+1].len() {
                let s0 = sync2d[i-1][sync_idx];
                let s1 = sync2d[i][sync_idx];
                let s2 = sync2d[i+1][sync_idx];

                // Fit parabola and find peak
                let denom = 2.0 * (s0 - 2.0 * s1 + s2);
                let is_peak = s1 > s0 && s1 > s2;

                // Debug for F5RXL frequency range (use RUST_LOG=trace)
                if tracing::enabled!(tracing::Level::TRACE) && i >= 407 && i <= 410 {
                    trace!(
                        bin = %i,
                        s0 = %s0,
                        s1 = %s1,
                        s2 = %s2,
                        is_peak = %is_peak,
                        denom = %denom,
                        "coarse interpolation debug"
                    );
                }

                if denom.abs() > 1e-6 && is_peak {
                    // Peak is at i, interpolate
                    let delta = 0.5 * (s2 - s0) / denom;
                    let interpolated = (i as f32 + delta) * df;

                    if tracing::enabled!(tracing::Level::TRACE) && i >= 407 && i <= 410 {
                        trace!(
                            bin = %i,
                            freq_before = %(i as f32 * df),
                            freq_after = %interpolated,
                            delta = %delta,
                            "interpolation result"
                        );
                    }

                    // Sanity check: should be within ±1 bin
                    if (interpolated - i as f32 * df).abs() <= df {
                        interpolated
                    } else {
                        i as f32 * df
                    }
                } else {
                    i as f32 * df
                }
            } else {
                i as f32 * df
            }
        } else {
            i as f32 * df
        };

        // Add candidate from narrow search
        if red > 0.0 {
            candidates.push(Candidate {
                frequency: interpolated_freq,
                time_offset: (jpeak as f32 - 0.5) * tstep,
                sync_power: red,
                baseline_noise,
            });
        }

        // Add second candidate from wide search if different peak
        // Also apply interpolation to wide search
        if red2 > 0.0 && jpeak2 != jpeak {
            let interpolated_freq2 = if i > 0 && i < sync2d.len() - 1 {
                let sync_idx2 = (jpeak2 + MAX_LAG) as usize;
                if sync_idx2 < sync2d[i-1].len() && sync_idx2 < sync2d[i].len() && sync_idx2 < sync2d[i+1].len() {
                    let s0 = sync2d[i-1][sync_idx2];
                    let s1 = sync2d[i][sync_idx2];
                    let s2 = sync2d[i+1][sync_idx2];

                    let denom = 2.0 * (s0 - 2.0 * s1 + s2);
                    if denom.abs() > 1e-6 && s1 > s0 && s1 > s2 {
                        let delta = 0.5 * (s2 - s0) / denom;
                        let interpolated = (i as f32 + delta) * df;
                        if (interpolated - i as f32 * df).abs() <= df {
                            interpolated
                        } else {
                            i as f32 * df
                        }
                    } else {
                        i as f32 * df
                    }
                } else {
                    i as f32 * df
                }
            } else {
                i as f32 * df
            };

            candidates.push(Candidate {
                frequency: interpolated_freq2,
                time_offset: (jpeak2 as f32 - 0.5) * tstep,
                sync_power: red2,
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

        debug!(
            total_candidates = candidates.len(),
            baseline_40th = %baseline,
            "sync power normalization"
        );

        if tracing::enabled!(tracing::Level::TRACE) {
            // Show how our target frequencies normalize
            let debug_bins = [136, 201, 218, 246, 736, 763, 877, 919, 932, 973]; // The frequencies we're tracking
            for &bin in &debug_bins {
                let freq = bin as f32 * df;
                for cand in candidates.iter().filter(|c| ((c.frequency / df) as usize) == bin) {
                    let normalized = cand.sync_power / baseline;
                    trace!(
                        freq = %freq,
                        sync_raw = %cand.sync_power,
                        sync_normalized = %normalized,
                        "frequency normalization"
                    );
                }
            }
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
                } else if cand.sync_power < sync_min {
                    trace!(
                        freq = %cand.frequency,
                        sync = %cand.sync_power,
                        sync_min = %sync_min,
                        time = %cand.time_offset,
                        "2733 Hz candidate FILTERED (below sync_min)"
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
