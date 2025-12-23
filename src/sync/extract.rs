///! Symbol extraction and LLR computation
///!
///! Extracts FT8 symbols from downsampled signal and computes log-likelihood ratios.

use super::Candidate;
use super::downsample::downsample_200hz;
use super::fine::sync_downsampled;
use super::fft::fft_real;
use super::COSTAS_PATTERN;
use tracing::trace;

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

/// Build per-symbol frequency tweak vector (ctwk) matching WSJT-X ft8b.f90:122-127
///
/// WSJT-X applies frequency correction per-symbol (32 samples) during sync correlation
/// rather than to the entire buffer. This is more accurate because:
/// 1. Phase is reset at each symbol boundary during sync measurement
/// 2. Avoids accumulating phase errors over 3200 samples
///
/// # Arguments
/// * `freq_offset_hz` - Frequency offset to correct in Hz
/// * `sample_rate` - Downsampled sample rate (typically ~200 Hz)
///
/// # Returns
/// 32-element complex tweak vector
fn build_ctwk(freq_offset_hz: f32, sample_rate: f32) -> [(f32, f32); 32] {
    let twopi = 2.0 * core::f32::consts::PI;
    let dt2 = 1.0 / sample_rate; // Sample period
    let dphi = twopi * freq_offset_hz * dt2;

    let mut ctwk = [(0.0f32, 0.0f32); 32];
    let mut phi = 0.0f32;

    for i in 0..32 {
        ctwk[i] = (f32::cos(phi), f32::sin(phi));
        phi = (phi + dphi) % twopi;
    }

    ctwk
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
/// Ok(nsync) on success with Costas sync quality (0-21), Err() if extraction fails
pub fn extract_symbols(
    signal: &[f32],
    candidate: &Candidate,
    nsym: usize,
    llr: &mut [f32],
) -> Result<usize, String> {
    extract_symbols_impl(signal, candidate, nsym, llr, None, None)
}

/// Internal implementation that optionally captures s8 power array
fn extract_symbols_impl(
    signal: &[f32],
    candidate: &Candidate,
    nsym: usize,
    llr: &mut [f32],
    mut llr_ratio_out: Option<&mut [f32]>,  // Optional ratio LLR output (mut for reborrowing)
    s8_out: Option<&mut [[f32; 79]; 8]>,
) -> Result<usize, String> {
    // Debug output disabled for performance
    // eprintln!("EXTRACT: freq={:.1} Hz, dt={:.2}s, nsym={}",
    //           candidate.frequency, candidate.time_offset, nsym);

    if llr.len() < 174 {
        return Err(format!("LLR buffer too small"));
    }
    if let Some(ref llr_ratio) = llr_ratio_out {
        if llr_ratio.len() < 174 {
            return Err(format!("LLR ratio buffer too small"));
        }
    }
    if nsym < 1 || nsym > 3 {
        return Err(format!("nsym must be 1, 2, or 3"));
    }
    const NN: usize = 79; // Number of FT8 symbols
    const SYMBOL_DURATION: f32 = 0.16; // FT8 symbol duration in seconds
    const NFFT_SYM: usize = 32; // FFT size for symbol extraction (power of 2)

    // Downsample centered on the refined frequency from fine_sync
    // Buffer size must match NFFT_OUT in downsample.rs (3200)
    let mut cd = vec![(0.0f32, 0.0f32); 3200];
    let actual_sample_rate = downsample_200hz(signal, candidate.frequency, &mut cd)?;

    // Convert time offset to sample index
    // candidate.time_offset is RELATIVE to 0.5s start (FT8 convention: dt=0 means signal starts at t=0.5s)
    // Add 0.5s to convert to absolute position in the downsampled buffer
    // Match WSJT-X nint() - round to nearest integer, not truncate
    let time_offset_samples = ((candidate.time_offset + 0.5) * actual_sample_rate).round() as i32;

    let mut best_correction = 0.0f32;

    // Only do fine phase correction for nsym=2/3 (nsym=1 doesn't need phase coherence)
    if nsym >= 2 {
        // Initial sync without correction
        let initial_sync = sync_downsampled(&cd, time_offset_samples, None, false, Some(actual_sample_rate));
        let mut best_sync = initial_sync;

        // Search ±1.0 Hz in 0.05 Hz steps using per-symbol ctwk (matching WSJT-X sync8d)
        // WSJT-X ft8b.f90:119-133 uses ctwk to apply frequency correction during sync
        // measurement rather than to the whole buffer. This is more accurate because
        // phase is reset at each symbol boundary.
        for correction_idx in -20..=20 {
            let freq_correction = correction_idx as f32 * 0.05; // ±1.0 Hz in 0.05 Hz steps

            if correction_idx == 0 {
                continue; // Already tested initial
            }

            // Build per-symbol frequency tweak vector (like WSJT-X ft8b.f90:122-127)
            let ctwk = build_ctwk(freq_correction, actual_sample_rate);

            // Test sync quality with per-symbol tweak applied during correlation
            let sync = sync_downsampled(&cd, time_offset_samples, Some(&ctwk), true, Some(actual_sample_rate));

            if sync > best_sync {
                best_sync = sync;
                best_correction = freq_correction;
            }
        }

        // Apply best correction to the whole buffer for symbol extraction
        // This is necessary because the FFT-based symbol extraction expects the
        // signal to be centered at baseband
        if best_correction.abs() > 0.001 {
            apply_phase_correction(&mut cd, best_correction, actual_sample_rate);
        }
    }

    // Calculate samples per symbol based on actual sample rate
    let nsps_down = (actual_sample_rate * SYMBOL_DURATION).round() as usize;

    // Convert time offset to sample index
    // candidate.time_offset is RELATIVE to 0.5s start (FT8 convention)
    // Add 0.5s to convert to absolute position in the downsampled buffer
    // Match WSJT-X nint() - round to nearest integer
    let initial_offset = ((candidate.time_offset + 0.5) * actual_sample_rate).round() as i32;

    // Timing refinement ±10 samples
    // This helps weak signals where fine_sync may have found a sub-optimal timing
    // due to different sample rate or rounding. Use sync_downsampled for consistency.
    // Note: WSJT-X does ±4 samples but we use ±10 to compensate for any accumulated
    // timing errors from the downsampling/frequency correction process.
    let mut best_offset = initial_offset;
    let mut best_sync = sync_downsampled(&cd, initial_offset, None, false, Some(actual_sample_rate));

    for dt in -10..=10 {
        if dt == 0 {
            continue; // Already computed
        }
        let t_offset = initial_offset + dt;
        let sync = sync_downsampled(&cd, t_offset, None, false, Some(actual_sample_rate));

        if sync > best_sync {
            best_sync = sync;
            best_offset = t_offset;
        }
    }

    let start_offset = best_offset;

    // MATCH WSJT-X: Allow negative start_offset (sync8d.f90 lines 43-46)
    // WSJT-X checks bounds per-symbol and sets out-of-bounds symbols to zero
    // This allows decoding signals that start before the recording (negative DT)
    // Example: F5RXL @ -0.77s has start_offset=-54, but Costas 2 & 3 are still in bounds!
    //
    // The per-symbol bounds check at line 264 handles out-of-bounds symbols correctly

    // Extract complex symbol values: cs[tone][symbol] for 8 tones × 79 symbols
    // Store COMPLEX values for multi-symbol soft decoding
    let mut cs = vec![[(0.0f32, 0.0f32); NN]; 8];
    let mut s8 = vec![[0.0f32; NN]; 8];

    // FFT buffers
    let mut sym_real = [0.0f32; NFFT_SYM];
    let mut sym_imag = [0.0f32; NFFT_SYM];

    // Match WSJT-X: no FFT window offset (ft8b.f90:157)
    // Copy samples directly without centering
    let fft_offset = 0;

    for k in 0..NN {
        // Symbol starts at: start_offset + k * nsps_down samples
        let i1 = start_offset + (k as i32) * (nsps_down as i32);
        let i2 = i1 + (nsps_down as i32);

        // Zero the FFT buffer
        for j in 0..NFFT_SYM {
            sym_real[j] = 0.0;
            sym_imag[j] = 0.0;
        }

        // Handle partial symbols (WSJT-X does this - zero-pads out-of-bounds regions)
        if i2 <= 0 || i1 >= cd.len() as i32 {
            // Symbol completely out of bounds - stays zero (already zeroed above)
            for tone in 0..8 {
                cs[tone][k] = (0.0, 0.0);
                s8[tone][k] = 0.0;
            }
            continue;
        }

        // Copy in-bounds portion of symbol, zero-padding out-of-bounds regions
        for j in 0..nsps_down {
            let idx = i1 + (j as i32);
            if idx >= 0 && (idx as usize) < cd.len() {
                let fft_idx = j + fft_offset;
                if fft_idx < NFFT_SYM {
                    sym_real[fft_idx] = cd[idx as usize].0;
                    sym_imag[fft_idx] = cd[idx as usize].1;
                }
            }
            // else: stays zero (out of bounds sample)
        }

        // Perform FFT
        fft_real(&mut sym_real, &mut sym_imag, NFFT_SYM)?;

        // Store COMPLEX values and magnitude for 8 tones
        // Use bins 0-7 for DC-centered signal
        // WSJT-X ft8b.f90:159-160:
        // cs(0:7,k)=csymb(1:8)/1e3     <- normalized by 1000
        // s8(0:7,k)=abs(csymb(1:8))    <- NOT normalized (used for Costas check)
        const NORM_FACTOR: f32 = 1000.0;

        for tone in 0..8 {
            let re = sym_real[tone];
            let im = sym_imag[tone];

            // Store normalized complex values for coherent combining
            cs[tone][k] = (re / NORM_FACTOR, im / NORM_FACTOR);
            // Store UNNORMALIZED magnitude for Costas validation
            s8[tone][k] = (re * re + im * im).sqrt();
        }
    }

    // Copy s8 to output if requested
    if let Some(s8_output) = s8_out {
        for tone in 0..8 {
            for k in 0..79 {
                s8_output[tone][k] = s8[tone][k];
            }
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
    // Using lenient threshold for now to attempt decoding marginal candidates
    // TODO: Match WSJT-X threshold of nsync > 6 once symbol extraction is fixed
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

                    // Standard difference method LLR (current method)
                    llr[bit_idx] = max_mag_1 - max_mag_0;

                    // Ratio method LLR (WSJT-X llrd equivalent)
                    if let Some(ref mut llr_ratio) = llr_ratio_out {
                        let den = max_mag_1.max(max_mag_0);
                        llr_ratio[bit_idx] = if den > 0.0 {
                            (max_mag_1 - max_mag_0) / den
                        } else {
                            0.0
                        };
                    }

                    bit_idx += 1;
                }

                k += nsym; // Move to next symbol (or group)
            } else if nsym == 3 {
                // Multi-symbol decoding: coherent combining (matches WSJT-X)
                // NOTE: Currently disabled in decoder due to phase drift issues
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

                    // Standard difference method LLR
                    llr[bit_idx] = max_mag_1 - max_mag_0;

                    // Ratio method LLR (WSJT-X llrd equivalent)
                    if let Some(ref mut llr_ratio) = llr_ratio_out {
                        let den = max_mag_1.max(max_mag_0);
                        llr_ratio[bit_idx] = if den > 0.0 {
                            (max_mag_1 - max_mag_0) / den
                        } else {
                            0.0
                        };
                    }

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

                        // Standard difference method LLR
                        llr[bit_idx] = max_mag_1 - max_mag_0;

                        // Ratio method LLR (WSJT-X llrd equivalent)
                        if let Some(ref mut llr_ratio) = llr_ratio_out {
                            let den = max_mag_1.max(max_mag_0);
                            llr_ratio[bit_idx] = if den > 0.0 {
                                (max_mag_1 - max_mag_0) / den
                            } else {
                                0.0
                            };
                        }

                        bit_idx += 1;
                    }

                    k += nsym;
                    continue;
                }

                // Two-symbol decoding for regular pairs (k=1,3,5,...,27)
                for i in 0..nt {
                    let i2 = (i / 8) % 8; // First symbol's 3-bit index (0-7)
                    let i3 = i % 8;       // Second symbol's 3-bit index (0-7)

                    let tone2 = GRAY_MAP[i2] as usize;
                    let tone3 = GRAY_MAP[i3] as usize;
                    let (r2, im2) = cs[tone2][ks];
                    let (r3, im3) = cs[tone3][ks + 1];

                    // Coherent combining (matches WSJT-X)
                    // NOTE: Currently disabled in decoder due to phase drift issues
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

                    // Standard difference method LLR
                    llr[bit_idx] = max_mag_1 - max_mag_0;

                    // Ratio method LLR (WSJT-X llrd equivalent)
                    if let Some(ref mut llr_ratio) = llr_ratio_out {
                        let den = max_mag_1.max(max_mag_0);
                        llr_ratio[bit_idx] = if den > 0.0 {
                            (max_mag_1 - max_mag_0) / den
                        } else {
                            0.0
                        };
                    }

                    bit_idx += 1;
                }

                k += nsym; // Move to next group
            } else {
                // Invalid nsym value
                break;
            }
        }
    }

    // Normalize difference method LLRs by standard deviation (match WSJT-X normalizebmet)
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

    // Normalize ratio method LLRs (if provided)
    if let Some(ref mut llr_ratio) = llr_ratio_out {
        let mut sum_r = 0.0f32;
        let mut sum_sq_r = 0.0f32;
        for i in 0..174 {
            sum_r += llr_ratio[i];
            sum_sq_r += llr_ratio[i] * llr_ratio[i];
        }
        let mean_r = sum_r / 174.0;
        let mean_sq_r = sum_sq_r / 174.0;
        let variance_r = mean_sq_r - mean_r * mean_r;
        let std_dev_r = if variance_r > 0.0 {
            variance_r.sqrt()
        } else {
            mean_sq_r.sqrt()
        };

        if std_dev_r > 0.0 {
            for i in 0..174 {
                llr_ratio[i] /= std_dev_r;
            }
        }

        // Then scale by WSJT-X scalefac=2.83
        for i in 0..174 {
            llr_ratio[i] *= 2.83;
        }
    }

    // Debug output disabled for performance
    // let llr_mean = llr.iter().map(|x| x.abs()).sum::<f32>() / llr.len() as f32;
    // let llr_max = llr.iter().map(|x| x.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
    // eprintln!("  Extracted: nsync={}/21, mean_abs_LLR={:.2}, max_LLR={:.2}",
    //           nsync, llr_mean, llr_max);

    Ok(nsync)
}

/// Calculate baseline noise floor from symbol power array
///
/// Computes noise floor by taking 20th percentile of average tone power per symbol
fn calculate_noise_baseline(s8: &[[f32; 79]; 8]) -> f64 {
    // For each symbol, compute average power across all 8 tones
    let mut avg_powers: Vec<f64> = Vec::with_capacity(79);
    for k in 0..79 {
        let mut sum = 0.0f64;
        for tone in 0..8 {
            sum += (s8[tone][k] as f64).powi(2);
        }
        avg_powers.push(sum / 8.0);
    }

    // Sort and take 20th percentile as noise floor estimate
    avg_powers.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = (avg_powers.len() as f32 * 0.20) as usize;
    avg_powers[idx.min(avg_powers.len() - 1)]
}

/// Calculate SNR from symbol powers and decoded tones following WSJT-X algorithm
///
/// # Arguments
/// * `s8` - Power array [8 tones x 79 symbols]
/// * `tones` - Decoded tone sequence (79 values, 0-7)
/// * `baseline_noise` - Optional baseline noise power from average spectrum (linear scale)
///
/// # Returns
/// SNR in dB, clamped to [-24, 30] range
pub fn calculate_snr(s8: &[[f32; 79]; 8], tones: &[u8; 79], baseline_noise: Option<f32>) -> i32 {
    // WSJT-X algorithm from ft8b.f90 computes two SNR estimates:
    //   xsnr  = 10*log10(xsig/xnoi - 1) - 27.0   (signal/off-tone)
    //   xsnr2 = 10*log10(xsig/xbase/3e6 - 1) - 27.0  (signal/baseline)

    let mut xsig = 0.0f64;
    let mut xnoi = 0.0f64;

    for i in 0..79 {
        let tone = tones[i] as usize;
        if tone < 8 {
            // Signal power at the decoded tone
            xsig += s8[tone][i] as f64 * s8[tone][i] as f64;

            // Noise power at opposite tone (4 away, mod 7 per WSJT-X)
            let off_tone = (tone + 4) % 7;
            xnoi += s8[off_tone][i] as f64 * s8[off_tone][i] as f64;
        }
    }

    // Method 1: Signal/off-tone ratio (xsnr)
    let xsnr = if xnoi > 1e-12 && xsig > xnoi {
        let arg = xsig / xnoi - 1.0;
        if arg > 0.1 {
            10.0 * arg.log10() - 27.0
        } else {
            -24.0
        }
    } else {
        -24.0
    };

    // Method 2: Signal/baseline ratio (xsnr2) if baseline is available
    let xsnr2 = if let Some(xbase) = baseline_noise {
        // WSJT-X formula: xsnr2 = 10*log10(xsig/xbase/scale - 1) - 27.0
        // Our xbase comes from 12 kHz spectrogram (sum of NHSYM=372 FFTs)
        // Our xsig comes from 200 Hz downsampled FFT with NFFT=32
        // Scale factor needs to account for:
        // - Different FFT sizes (WSJT-X uses larger FFTs)
        // - Different integration times
        // - Empirically determined to match WSJT-X SNR measurements
        let scale_factor = 0.05;  // Adjusted from 10.0 to correct 23 dB underestimation
        let xbase_scaled = xbase as f64 * scale_factor;

        if xbase_scaled > 1e-30 && xsig > xbase_scaled * 0.1 {
            let arg = xsig / xbase_scaled - 1.0;
            if arg > 0.1 {
                10.0 * arg.log10() - 27.0
            } else {
                -24.0
            }
        } else {
            -24.0
        }
    } else {
        -24.0
    };

    // Use signal/off-tone ratio method (xsnr) - the baseline method has scaling issues
    // TODO: Fix baseline_noise scaling to match WSJT-X units
    let snr = xsnr;

    // Clamp to reasonable range
    snr.max(-24.0).min(30.0) as i32
}

/// Extract symbols and symbol powers for SNR calculation
///
/// Returns both LLRs for decoding and s8 power array for SNR estimation
pub fn extract_symbols_with_powers(
    signal: &[f32],
    candidate: &Candidate,
    nsym: usize,
    llr: &mut [f32],
    s8_out: &mut [[f32; 79]; 8],
) -> Result<usize, String> {
    extract_symbols_impl(signal, candidate, nsym, llr, None, Some(s8_out))
}

/// Extract symbols with DUAL LLR methods (difference and ratio)
///
/// Computes both standard difference LLR (max_1 - max_0) and normalized ratio LLR
/// ((max_1 - max_0) / max(max_1, max_0)) in a single pass. This matches WSJT-X's
/// 4-pass strategy where llra uses difference method and llrd uses ratio method.
///
/// The ratio method provides a normalized LLR that's more robust to amplitude variations.
pub fn extract_symbols_dual_llr(
    signal: &[f32],
    candidate: &Candidate,
    nsym: usize,
    llr_diff: &mut [f32],
    llr_ratio: &mut [f32],
    s8_out: &mut [[f32; 79]; 8],
) -> Result<usize, String> {
    extract_symbols_impl(signal, candidate, nsym, llr_diff, Some(llr_ratio), Some(s8_out))
}

/// Extract symbols with ALL FOUR LLR methods (matching WSJT-X ft8b.f90)
///
/// WSJT-X generates 4 separate LLR arrays and normalizes each independently:
/// - llra (bmeta): nsym=1, difference method
/// - llrb (bmetb): nsym=2, difference method
/// - llrc (bmetc): nsym=3, difference method
/// - llrd (bmetd): nsym=1, ratio method
///
/// Each array is normalized independently before scaling by 2.83 (WSJT-X scalefac).
/// The decoder then tries all 4 methods with multiple scaling factors.
///
/// # Arguments
/// * `signal` - Input signal (15 seconds at 12 kHz)
/// * `candidate` - Refined candidate from fine_sync
/// * `llra` - Output: nsym=1 difference LLR (174 values)
/// * `llrb` - Output: nsym=2 difference LLR (174 values)
/// * `llrc` - Output: nsym=3 difference LLR (174 values)
/// * `llrd` - Output: nsym=1 ratio LLR (174 values)
/// * `s8_out` - Output: Symbol powers for SNR calculation (8×79)
///
/// # Returns
/// Ok(nsync) on success with Costas sync quality (0-21), Err() if extraction fails
pub fn extract_symbols_all_llr(
    signal: &[f32],
    candidate: &Candidate,
    llra: &mut [f32],  // nsym=1 difference
    llrb: &mut [f32],  // nsym=2 difference
    llrc: &mut [f32],  // nsym=3 difference
    llrd: &mut [f32],  // nsym=1 ratio
    s8_out: &mut [[f32; 79]; 8],
) -> Result<usize, String> {
    // Extract nsym=1 with both difference and ratio methods
    // Capture nsync from the first extraction (all extractions use same signal, so nsync is same)
    // NOTE: extract_symbols_impl already normalizes and scales LLRs internally,
    // so we DO NOT apply normalization or scaling here to avoid double-processing
    let nsync = extract_symbols_impl(signal, candidate, 1, llra, Some(llrd), Some(s8_out))?;

    // Extract nsym=2 (difference only, ratio not used by WSJT-X for nsym>1)
    extract_symbols_impl(signal, candidate, 2, llrb, None, None)?;

    // Extract nsym=3 (difference only, ratio not used by WSJT-X for nsym>1)
    extract_symbols_impl(signal, candidate, 3, llrc, None, None)?;

    // LLRs are already normalized and scaled by extract_symbols_impl (lines 642-696)
    // Do NOT apply additional normalization or scaling here!

    Ok(nsync)
}

/// Normalize LLR array (matching WSJT-X normalizebmet subroutine)
///
/// WSJT-X normalizebmet divides by standard deviation WITHOUT centering.
/// This is critical - centering changes the LLR distribution and breaks decoding!
fn normalize_llr(llr: &mut [f32]) {
    let n = llr.len();
    if n == 0 {
        return;
    }

    // Calculate mean
    let sum: f32 = llr.iter().sum();
    let mean = sum / n as f32;

    // Calculate mean of squares
    let sum_sq: f32 = llr.iter().map(|&x| x * x).sum();
    let mean_sq = sum_sq / n as f32;

    // Calculate variance: var = E[X^2] - E[X]^2
    let var = mean_sq - mean * mean;

    // Calculate standard deviation
    let std = if var > 0.0 {
        var.sqrt()
    } else {
        mean_sq.sqrt()
    };

    // Normalize by std (WSJT-X does NOT subtract mean!)
    if std > 1e-6 {
        for val in llr.iter_mut() {
            *val /= std;
        }
    }
}

/// Estimate frequency offset from Costas array phase progression
///
/// Measures the phase of Costas tones and calculates frequency offset
/// from phase drift over time. This enables sub-0.1 Hz frequency accuracy
/// by using the phase progression of successfully extracted Costas arrays.
///
/// # Algorithm
///
/// FT8 has 3 Costas arrays at known positions:
/// - Costas 1: symbols 0-6
/// - Costas 2: symbols 36-42
/// - Costas 3: symbols 72-78
///
/// If frequency is off by Δf, phase drifts linearly: Δφ = 2π × Δf × Δt
/// Therefore: Δf = Δφ / (2π × Δt)
///
/// # Arguments
///
/// * `signal` - Raw 12 kHz input signal
/// * `candidate` - Current candidate with initial frequency estimate
///
/// # Returns
///
/// * `Ok(refined_frequency)` if Costas sync is good enough (≥5 tones per array)
/// * `Err(message)` if Costas sync is poor or phase measurement fails
///
/// # Example
///
/// ```ignore
/// // After initial extraction shows good Costas sync (nsync >= 15/21)
/// if let Ok(refined_freq) = estimate_frequency_from_phase(signal, candidate) {
///     let correction = refined_freq - candidate.frequency;
///     if correction.abs() < 1.0 && correction.abs() > 0.01 {
///         // Re-extract at refined frequency
///     }
/// }
/// ```
pub fn estimate_frequency_from_phase(
    signal: &[f32],
    candidate: &Candidate,
) -> Result<f32, String> {
    // Downsample at current frequency estimate
    let mut cd = vec![(0.0f32, 0.0f32); 3200];
    downsample_200hz(signal, candidate.frequency, &mut cd)?;

    const NSPS: usize = 32; // 200 Hz × 0.16s = 32 samples per symbol
    const NFFT_SYM: usize = 32;

    // Calculate start offset in downsampled signal
    // candidate.time_offset is RELATIVE to 0.5s start (FT8 convention)
    // Add 0.5s to convert to absolute position in the downsampled buffer
    let dt = candidate.time_offset + 0.5;
    let start_offset = (dt * 200.0) as i32; // Convert to sample index

    // Measure phase for each of 3 Costas arrays
    let mut costas_data: Vec<(usize, f32, usize)> = Vec::new(); // (start_idx, phase, valid_count)

    for costas_start in [0, 36, 72] {
        let mut phase_sum = 0.0;
        let mut weight_sum = 0.0;
        let mut valid_tones = 0;

        // Extract phase from each of 7 Costas tones
        for k in 0..7 {
            let symbol_idx = costas_start + k;
            let expected_tone = COSTAS_PATTERN[k];
            let i1 = start_offset + (symbol_idx as i32) * (NSPS as i32);

            // Check bounds
            if i1 < 0 || (i1 as usize + NSPS) > cd.len() {
                continue;
            }

            // Extract symbol
            let mut sym_real = [0.0f32; NFFT_SYM];
            let mut sym_imag = [0.0f32; NFFT_SYM];

            for j in 0..NSPS {
                let idx = (i1 as usize) + j;
                sym_real[j] = cd[idx].0;
                sym_imag[j] = cd[idx].1;
            }

            // Perform FFT
            if fft_real(&mut sym_real, &mut sym_imag, NFFT_SYM).is_err() {
                continue;
            }

            // Get phase at expected tone bin
            let tone_bin = expected_tone as usize;
            let re = sym_real[tone_bin];
            let im = sym_imag[tone_bin];
            let power = re * re + im * im;

            if power > 0.001 {
                let phase = im.atan2(re);
                phase_sum += phase * power; // Weighted by power
                weight_sum += power;
                valid_tones += 1;
            }
        }

        // Average phase for this Costas array
        if valid_tones >= 5 && weight_sum > 0.0 {
            let avg_phase = phase_sum / weight_sum;
            costas_data.push((costas_start, avg_phase, valid_tones));
            trace!(
                costas_idx = costas_data.len(),
                symbols_start = costas_start,
                symbols_end = costas_start + 6,
                valid_tones,
                avg_phase,
                "Costas array phase measurement"
            );
        }
    }

    // Need at least 2 Costas arrays for phase drift measurement
    if costas_data.len() < 2 {
        return Err(format!("Not enough Costas arrays detected: {}", costas_data.len()));
    }

    // Find the best pair of Costas arrays (prefer those with all 7 tones valid)
    // Prioritize: (1) both have 7/7 tones, (2) largest separation in time
    let mut best_pair: Option<(usize, usize)> = None;
    let mut best_quality = 0;
    let mut best_separation = 0;

    for i in 0..costas_data.len() {
        for j in (i+1)..costas_data.len() {
            let (start_i, _, count_i) = costas_data[i];
            let (start_j, _, count_j) = costas_data[j];

            // Quality: sum of valid tone counts (max 14 for 7+7)
            let quality = count_i + count_j;
            let separation = start_j - start_i;

            // Prefer pairs with higher quality, then larger separation
            if quality > best_quality || (quality == best_quality && separation > best_separation) {
                best_quality = quality;
                best_separation = separation;
                best_pair = Some((i, j));
            }
        }
    }

    let (idx1, idx2) = best_pair.ok_or("No valid Costas pair found")?;
    let (start1, phase1, count1) = costas_data[idx1];
    let (start2, phase2, count2) = costas_data[idx2];

    // Calculate phase differences (handle wrapping)
    let unwrap_phase = |p1: f32, p2: f32| -> f32 {
        let mut dp = p2 - p1;
        if dp > std::f32::consts::PI {
            dp -= 2.0 * std::f32::consts::PI;
        } else if dp < -std::f32::consts::PI {
            dp += 2.0 * std::f32::consts::PI;
        }
        dp
    };

    // Calculate phase drift between the best pair
    let phase_drift = unwrap_phase(phase1, phase2);

    // Time between Costas arrays (in seconds)
    let symbol_separation = (start2 - start1) as f32; // in symbols
    const SYMBOL_DURATION: f32 = 0.16; // seconds per symbol
    let time_separation = symbol_separation * SYMBOL_DURATION;

    trace!(
        pair_start1 = start1,
        pair_end1 = start1 + 6,
        pair_start2 = start2,
        pair_end2 = start2 + 6,
        quality = best_quality,
        separation_symbols = symbol_separation as usize,
        phase_drift,
        time_separation,
        "Using Costas pair for phase measurement"
    );

    // Calculate frequency offset: Δf = Δφ / (2π × Δt)
    let freq_offset = phase_drift / (2.0 * std::f32::consts::PI * time_separation);

    trace!(
        phase_drift,
        time_separation,
        freq_offset,
        "Frequency offset from phase drift"
    );

    // Sanity check: offset should be < 2 Hz for typical fine sync errors
    if freq_offset.abs() > 2.0 {
        return Err(format!("Unrealistic frequency offset: {:.3} Hz", freq_offset));
    }

    let refined_freq = candidate.frequency + freq_offset;

    Ok(refined_freq)
}
