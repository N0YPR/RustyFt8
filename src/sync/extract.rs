///! Symbol extraction and LLR computation
///!
///! Extracts FT8 symbols from downsampled signal and computes log-likelihood ratios.

use super::candidate::Candidate;
use super::downsample::downsample_200hz;
use super::fine::sync_downsampled;
use super::fft::fft_real;
use super::COSTAS_PATTERN;

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
            let power = (re * re + im * im).sqrt();

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

/// Apply phase correction to remove residual frequency offset (like WSJT-X twkfreq1)
///
/// This maintains phase coherence between symbols for nsym=2/3 coherent combining
fn apply_phase_correction(cd: &mut [(f32, f32)], freq_offset_hz: f32, sample_rate: f32) {
    let dphi = 2.0 * core::f32::consts::PI * freq_offset_hz / sample_rate;
    let mut phi = 0.0f32;

    for i in 0..cd.len() {
        let cos_phi = f32::cos(phi);
        let sin_phi = f32::sin(phi);

        // Complex multiplication: cd[i] *= exp(j*phi)
        let (re, im) = cd[i];
        cd[i] = (
            re * cos_phi - im * sin_phi,
            re * sin_phi + im * cos_phi,
        );

        phi += dphi;
        if phi > core::f32::consts::PI {
            phi -= 2.0 * core::f32::consts::PI;
        }
    }
}

/// Extract 174 LLR values using multi-symbol soft decoding.
///
/// This function:
/// 1. Downsamples signal to ~200 Hz centered on candidate frequency
/// 2. Applies fine phase correction for nsym >= 2
/// 3. Refines time offset to align with symbol boundaries
/// 4. Extracts 79 symbols via FFT (8 tones per symbol)
/// 5. Computes soft LLRs for 174 information bits using nsym-symbol combining
///
/// # Arguments
/// * `signal` - Input signal (15 seconds at 12 kHz)
/// * `candidate` - Refined candidate from fine_sync
/// * `nsym` - Number of symbols to combine (1, 2, or 3)
/// * `llr` - Output buffer for 174 log-likelihood ratios
///
/// # Returns
/// Ok(()) on success, Err() if extraction fails
pub fn extract_symbols(
    signal: &[f32],
    candidate: &Candidate,
    nsym: usize,
    llr: &mut [f32],
) -> Result<(), String> {
    if llr.len() < 174 {
        return Err(format!("LLR buffer too small"));
    }
    if nsym < 1 || nsym > 3 {
        return Err(format!("nsym must be 1, 2, or 3"));
    }
    const NN: usize = 79; // Number of FT8 symbols
    const SYMBOL_DURATION: f32 = 0.16; // FT8 symbol duration in seconds
    const NFFT_SYM: usize = 32; // FFT size for symbol extraction (power of 2)

    // Downsample centered on the refined frequency from fine_sync
    let mut cd = vec![(0.0f32, 0.0f32); 4096];
    let actual_sample_rate = downsample_200hz(signal, candidate.frequency, &mut cd)?;

    // CRITICAL for nsym=2/3: Apply fine phase correction to remove residual frequency offset
    // Even 0.1 Hz error causes phase drift that decorrelates adjacent symbols
    // Search ±0.3 Hz with 0.05 Hz resolution to find optimal phase tracking
    let time_offset_samples = ((candidate.time_offset + 0.5) * actual_sample_rate) as i32;

    let mut best_correction = 0.0f32;

    // Only do fine phase correction for nsym=2/3 (nsym=1 doesn't need phase coherence)
    if nsym >= 2 {
        let mut cd_test = cd.clone();

        // Initial sync without correction
        let initial_sync = sync_downsampled(&cd, time_offset_samples, None, false, Some(actual_sample_rate));
        let mut best_sync = initial_sync;

        for correction_idx in -6..=6 {
            let freq_correction = correction_idx as f32 * 0.05; // ±0.3 Hz in 0.05 Hz steps

            if correction_idx == 0 {
                continue; // Already tested initial
            }

            // Apply phase correction
            cd_test.copy_from_slice(&cd);
            apply_phase_correction(&mut cd_test, freq_correction, actual_sample_rate);

            // Test sync quality
            let sync = sync_downsampled(&cd_test, time_offset_samples, None, false, Some(actual_sample_rate));

            if sync > best_sync {
                best_sync = sync;
                best_correction = freq_correction;
            }
        }

        // Apply best correction to working buffer
        if best_correction.abs() > 0.001 {
            apply_phase_correction(&mut cd, best_correction, actual_sample_rate);
        }
    }

    // Calculate samples per symbol based on actual sample rate
    let nsps_down = (actual_sample_rate * SYMBOL_DURATION).round() as usize;

    // Convert time offset to sample index and refine it locally
    let initial_offset = ((candidate.time_offset + 0.5) * actual_sample_rate) as i32;


    // Do a comprehensive fine time search to find optimal symbol timing
    // Search over a wider range to account for timing drift and downsampling artifacts
    let mut best_offset = initial_offset;
    let mut best_metric = 0.0f32;

    // Search range: ±10 samples (±53ms at 187.5 Hz, ±1/3 symbol period)
    for dt in -10..=10 {
        let t_offset = initial_offset + dt;

        // Compute sync metric based on Costas array strength
        let sync = sync_downsampled(&cd, t_offset, None, false, Some(actual_sample_rate));

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


    // Extract complex symbol values: cs[tone][symbol] for 8 tones × 79 symbols
    // Store COMPLEX values for multi-symbol soft decoding
    let mut cs = vec![[(0.0f32, 0.0f32); NN]; 8];
    let mut s8 = vec![[0.0f32; NN]; 8];

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
            s8[tone][k] = (re * re + im * im).sqrt();
        }
    }

    // Validate Costas arrays (quality check)
    let mut nsync = 0;

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

        if max_tone == expected_tone as usize {
            nsync += 1;
        }
    }

    // If sync quality is too low, reject
    // Note: Temporarily lowered threshold for testing
    if nsync < 3 {
        return Err(format!("Sync quality too low: {}/21 Costas tones correct", nsync));
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

    // Multi-symbol soft decoding for improved SNR performance
    // nsym=1: 8 combinations, nsym=2: 64 combinations, nsym=3: 512 combinations
    // WSJT-X tries all three in multiple decoding passes
    let nt = 8_usize.pow(nsym as u32); // 8^nsym possible tone combinations

    // Process two data symbol blocks
    // Match WSJT-X: k=1..29 (Fortran 1-indexed), we use k=1..29 (adjusted for 0-indexing later)
    for ihalf in 0..2 {
        let base_offset = if ihalf == 0 { 7 } else { 43 };

        // CRITICAL: Start at k=1 (not k=0) to match WSJT-X!
        // For nsym=2: k=1,3,5,...,29 → pairs (8,9), (10,11), (12,13), ...
        // For nsym=1: k=1,2,3,...,29 → symbols 8,9,10,...,36
        let mut k = 1;
        while k <= 29 {
            if bit_idx >= 174 {
                break;
            }

            let ks = k + base_offset - 1; // k=1..29 (1-indexed), base_offset=7 or 43, result is 0-indexed symbol position
            let mut s2 = vec![0.0f32; nt]; // Magnitudes for all combinations

            if nsym == 1 {
                // Single-symbol decoding
                // For each tone (0-7), get its power and map to the 3-bit index it represents
                // s2[index] = power of the tone that decodes to that 3-bit index
                for tone in 0..8 {
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
                    for index in 0..8 {
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

                k += nsym; // Move to next symbol (or group)
            } else if nsym == 3 {
                // Multi-symbol decoding: coherently combine 3 symbols
                for i in 0..nt {
                    let i1 = i / 64; // First symbol's tone
                    let i2 = (i / 8) % 8; // Second symbol's tone
                    let i3 = i % 8; // Third symbol's tone

                    if ks + 2 < NN {
                        let (r1, im1) = cs[GRAY_MAP[i1] as usize][ks];
                        let (r2, im2) = cs[GRAY_MAP[i2] as usize][ks + 1];
                        let (r3, im3) = cs[GRAY_MAP[i3] as usize][ks + 2];

                        let sum_r = r1 + r2 + r3;
                        let sum_im = im1 + im2 + im3;
                        s2[i] = (sum_r * sum_r + sum_im * sum_im).sqrt();
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

                    for i in 0..nt {
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

                k += nsym; // Move to next group
            } else if nsym == 2 {
                // Two-symbol decoding: coherently combine 2 symbols
                // Special case: 29 symbols per half = 14 pairs + 1 leftover
                // Use single-symbol decoding for the last odd symbol (k=29)
                if k == 29 {
                    // Single-symbol decoding for last odd symbol
                    for tone in 0..8 {
                        let index = GRAY_MAP_INV[tone];
                        s2[index as usize] = s8[tone][ks];
                    }

                    // Extract 3 bits from this single symbol
                    const IBMAX_SINGLE: usize = 2;
                    for ib in 0..=IBMAX_SINGLE {
                        if bit_idx >= 174 {
                            break;
                        }

                        let bit_pos = IBMAX_SINGLE - ib;
                        let mut max_mag_1 = -1e30f32;
                        let mut max_mag_0 = -1e30f32;

                        for i in 0..8 {
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

                    k += nsym;
                    continue;
                }

                // Two-symbol decoding for regular pairs (k=1,3,5,...,27)

                for i in 0..nt {
                    let i2 = (i / 8) % 8; // First symbol's 3-bit index (0-7)
                    let i3 = i % 8;       // Second symbol's 3-bit index (0-7)

                    // Always combine the pair (may include sync symbols at boundaries)
                    let tone2 = GRAY_MAP[i2] as usize;
                    let tone3 = GRAY_MAP[i3] as usize;
                    let (r2, im2) = cs[tone2][ks];
                    let (r3, im3) = cs[tone3][ks + 1];

                    let sum_r = r2 + r3;
                    let sum_im = im2 + im3;
                    s2[i] = (sum_r * sum_r + sum_im * sum_im).sqrt();
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

                    for i in 0..nt {
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

                k += nsym; // Move to next group
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
        variance.sqrt()
    } else {
        mean_sq.sqrt()
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

    Ok(())
}
