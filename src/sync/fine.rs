///! Fine synchronization for FT8 signals
///!
///! Refines frequency and timing estimates from coarse sync.

use super::candidate::Candidate;
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

        if sync > best_sync {
            best_sync = sync;
            best_freq = test_freq;
        }
    }

    // eprintln!("  Freq search: best_freq={:.1} Hz, final_sync={:.3}", best_freq, best_sync);

    // Convert back to seconds (inverse of the initial_offset calculation)
    let refined_time = (best_time as f32 / actual_sample_rate) - 0.5;

    eprintln!("  REFINED: freq_in={:.1} -> freq_out={:.1} Hz, dt_out={:.2}s, sync_out={:.3}",
              candidate.frequency, best_freq, refined_time, best_sync);

    Ok(Candidate {
        frequency: best_freq,
        time_offset: refined_time,
        sync_power: best_sync,
        baseline_noise: candidate.baseline_noise, // Preserve baseline noise from coarse sync
    })
}
