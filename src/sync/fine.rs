///! Fine synchronization for FT8 signals
///!
///! Refines frequency and timing estimates from coarse sync.

use super::Candidate;
use super::downsample::downsample_200hz;
use super::COSTAS_PATTERN;

/// Compute sync power on downsampled signal using Costas correlation
///
/// This is the fine sync equivalent of compute_sync2d, operating on
/// downsampled data at 200 Hz (32 samples/symbol).
///
/// # Arguments
/// * `cd` - Downsampled complex signal
/// * `time_offset` - Time offset in samples
/// * `freq_tweak` - Optional frequency correction phasors (32 per symbol)
/// * `apply_tweak` - Whether to apply frequency correction
/// * `actual_rate` - Actual downsampled rate in Hz (default 200 if None)
///
/// # Returns
/// Sync power metric
pub fn sync_downsampled(
    cd: &[(f32, f32)],
    time_offset: i32,
    freq_tweak: Option<&[(f32, f32)]>,
    apply_tweak: bool,
    actual_rate: Option<f32>,
) -> f32 {
    const NSPS_DOWN: usize = 32; // Samples per symbol
    const FT8_TONE_SPACING: f32 = 6.25; // Hz

    let sample_rate = actual_rate.unwrap_or(200.0);

    // Precompute Costas waveforms at actual sample rate
    // Tone k is at frequency k * 6.25 Hz
    let mut costas_wave = [[(0.0f32, 0.0f32); NSPS_DOWN]; 7];

    for (i, &tone) in COSTAS_PATTERN.iter().enumerate() {
        let tone_freq_hz = tone as f32 * FT8_TONE_SPACING;
        let dphi = 2.0 * core::f32::consts::PI * tone_freq_hz / sample_rate;
        let mut phi = 0.0f32;

        for j in 0..NSPS_DOWN {
            costas_wave[i][j] = (f32::cos(phi), f32::sin(phi));
            phi = phi + dphi;
            if phi > 2.0 * core::f32::consts::PI {
                phi -= 2.0 * core::f32::consts::PI;
            }
        }
    }

    let mut sync = 0.0f32;
    let mut total_valid_costas = 0u32;  // Count valid Costas symbols across all tones

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
            total_valid_costas += 1;
        }

        if i2 >= 0 && (i2 as usize + NSPS_DOWN - 1) < cd.len() {
            for j in 0..NSPS_DOWN {
                let idx = i2 as usize + j;
                let (sr, si) = cd[idx];
                let (wr, wi) = wave[j];
                z2.0 += sr * wr + si * wi;
                z2.1 += si * wr - sr * wi;
            }
            total_valid_costas += 1;
        }

        if i3 >= 0 && (i3 as usize + NSPS_DOWN - 1) < cd.len() {
            for j in 0..NSPS_DOWN {
                let idx = i3 as usize + j;
                let (sr, si) = cd[idx];
                let (wr, wi) = wave[j];
                z3.0 += sr * wr + si * wi;
                z3.1 += si * wr - sr * wi;
            }
            total_valid_costas += 1;
        }

        // Add power from all three Costas arrays
        sync += z1.0 * z1.0 + z1.1 * z1.1;
        sync += z2.0 * z2.0 + z2.1 * z2.1;
        sync += z3.0 * z3.0 + z3.1 * z3.1;
    }

    // MATCH WSJT-X: Normalize by number of valid Costas symbols
    // For negative DT signals, Costas 1 (7 symbols) may be out of bounds
    // Without normalization, sync score is unfairly reduced by ~33%
    // This caused F5RXL (sync=38.69 coarse) to drop to sync=1.16 fine
    // Example: F5RXL with Costas 1 OOB: 14 valid symbols (7 tones × 2 arrays)
    //          Normalization: sync * 21/14 = sync * 1.5 (restores ~33% penalty)
    const TOTAL_COSTAS_SYMBOLS: f32 = 21.0;  // 7 tones * 3 arrays
    if total_valid_costas > 0 {
        sync = sync * TOTAL_COSTAS_SYMBOLS / (total_valid_costas as f32);
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
    eprintln!("FINE_SYNC: freq={:.1} Hz, dt_in={:.2}s, sync_in={:.3}",
              candidate.frequency, candidate.time_offset, candidate.sync_power);

    // Downsample centered on candidate frequency
    // Buffer size must match NFFT_OUT in downsample.rs (3200)
    let mut cd = vec![(0.0f32, 0.0f32); 3200];
    let actual_sample_rate = downsample_200hz(signal, candidate.frequency, &mut cd)?;

    // Diagnostic: check if downsampled buffer has data
    // let cd_power: f32 = cd.iter().take(100).map(|(r, i)| r*r + i*i).sum();
    // eprintln!("  Downsampled: rate={:.1} Hz, buffer_len={}, first_100_power={:.3}",
    //           actual_sample_rate, cd.len(), cd_power);

    // Convert time offset to downsampled sample index
    // candidate.time_offset is relative to 0.5s start, but downsampled buffer starts at 0.0
    // So add 0.5s to convert to absolute time, then multiply by actual sample rate
    let initial_offset = ((candidate.time_offset + 0.5) * actual_sample_rate) as i32;
    // eprintln!("  Initial offset: {} samples (from dt={:.2}s)", initial_offset, candidate.time_offset);


    // Fine time search: ±10 samples = ±50 ms (matching WSJT-X ft8b.f90:110)
    // WSJT-X searches i0-10 to i0+10, which is "over +/- one quarter symbol"
    let mut best_time = initial_offset;
    let mut best_sync = 0.0f32;

    for dt in -10..=10 {
        let t_offset = initial_offset + dt;
        let sync = sync_downsampled(&cd, t_offset, None, false, Some(actual_sample_rate));

        if sync > best_sync {
            best_sync = sync;
            best_time = t_offset;
        }
    }

    // eprintln!("  Time search: best_time_samples={}, best_sync={:.3}", best_time, best_sync);

    // Fine frequency search: ±2.5 Hz in 0.5 Hz steps (matching WSJT-X)
    // Unlike phase rotation, we RE-DOWNSAMPLE at each test frequency
    // This ensures perfect centering at baseband, critical for nsym=2
    let mut best_freq = candidate.frequency;
    let mut sync_scores: Vec<(f32, f32)> = Vec::with_capacity(11); // Store (freq, sync) pairs

    for df in -5..=5 {
        let freq_offset = df as f32 * 0.5; // 0.5 Hz steps
        let test_freq = candidate.frequency + freq_offset;

        // Re-downsample at the test frequency
        // Buffer size must match NFFT_OUT in downsample.rs (3200)
        let mut cd_test = vec![(0.0f32, 0.0f32); 3200];
        let test_rate = match downsample_200hz(signal, test_freq, &mut cd_test) {
            Ok(rate) => rate,
            Err(_) => continue,
        };

        let sync = sync_downsampled(&cd_test, best_time, None, false, Some(test_rate));
        sync_scores.push((test_freq, sync));

        if sync > best_sync {
            best_sync = sync;
            best_freq = test_freq;
        }
    }

    // Parabolic interpolation to refine best_freq beyond 0.5 Hz quantization
    // Improves accuracy from 0.5 Hz to ~0.05-0.1 Hz
    // This is critical for tone extraction: 0.2 Hz error causes 20% tone errors!
    if sync_scores.len() >= 3 {
        // Find the index of best_freq in sync_scores
        if let Some(best_idx) = sync_scores.iter().position(|(f, _)| (*f - best_freq).abs() < 0.01) {
            // Need neighbors for interpolation
            if best_idx > 0 && best_idx < sync_scores.len() - 1 {
                let (f0, s0) = sync_scores[best_idx - 1];
                let (f1, s1) = sync_scores[best_idx];
                let (f2, s2) = sync_scores[best_idx + 1];

                // Fit parabola: y = ax² + bx + c through (f0,s0), (f1,s1), (f2,s2)
                // Peak is at x = -b/(2a)
                // Using simplified formula for equally-spaced points:
                let denom = 2.0 * (s0 - 2.0 * s1 + s2);
                if denom.abs() > 1e-6 {  // Avoid division by zero
                    let delta = 0.5 * (s2 - s0) / denom;
                    let interpolated_freq = f1 + delta * 0.5; // delta is in units of 0.5 Hz steps

                    // Sanity check: interpolated frequency should be within ±0.5 Hz of discrete peak
                    if (interpolated_freq - f1).abs() <= 0.5 {
                        // eprintln!("  Parabolic interpolation (fine): {} Hz → {} Hz (Δ={:.3} Hz)",
                        //          f1, interpolated_freq, interpolated_freq - f1);
                        best_freq = interpolated_freq;
                    }
                }
            }
        }
    }

    // CRITICAL: Re-downsample at the best frequency for final time refinement
    // This matches WSJT-X ft8b.f90:140
    let final_sample_rate = downsample_200hz(signal, best_freq, &mut cd)?;

    // CRITICAL: Final time search ±4 samples after frequency correction
    // This matches WSJT-X ft8b.f90:144-150
    // "Search over +/- one quarter symbol" for final time alignment
    let mut final_best_time = best_time;
    let mut final_best_sync = 0.0f32;

    for dt in -4..=4 {
        let t_offset = best_time + dt;
        let sync = sync_downsampled(&cd, t_offset, None, false, Some(final_sample_rate));

        if sync > final_best_sync {
            final_best_sync = sync;
            final_best_time = t_offset;
        }
    }

    best_time = final_best_time;
    best_sync = final_best_sync;

    // eprintln!("  Freq search: best_freq={:.1} Hz, final_sync={:.3}", best_freq, best_sync);

    // Convert back to seconds (matching WSJT-X ft8b.f90 line 151)
    // WSJT-X: xdt=(ibest-1)*dt2, where dt2=1/fs2
    // Output is ABSOLUTE time from t=0, NOT relative to 0.5s
    // Use final_sample_rate since we re-downsampled at best_freq
    let refined_time = best_time as f32 / final_sample_rate;

    eprintln!("  REFINED: freq_in={:.1} -> freq_out={:.1} Hz, dt_out={:.2}s, sync_coarse={:.3} (preserved)",
              candidate.frequency, best_freq, refined_time, candidate.sync_power);

    // CRITICAL: Preserve coarse sync score (matching WSJT-X ft8b.f90)
    // WSJT-X uses fine sync ONLY to refine frequency and time, NOT for ranking
    // The coarse sync score (sync8.f90:124) is preserved for candidate selection
    // This prevents negative DT signals from being filtered out due to low fine sync scores
    Ok(Candidate {
        frequency: best_freq,
        time_offset: refined_time,
        sync_power: candidate.sync_power,  // ← PRESERVE coarse sync, don't use best_sync
        baseline_noise: candidate.baseline_noise, // Preserve baseline noise from coarse sync
    })
}
