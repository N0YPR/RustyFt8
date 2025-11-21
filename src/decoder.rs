//! Multi-signal FT8 decoder
//!
//! Implements the complete FT8 decode pipeline for processing recordings with multiple signals.
//! Follows WSJT-X architecture: scans for candidates, decodes each, reports immediately via callback.

use crate::{ldpc, symbol, sync};
use bitvec::prelude::*;
use rayon::prelude::*;

/// Decoded FT8 message with metadata
#[derive(Debug, Clone)]
pub struct DecodedMessage {
    /// The decoded message text
    pub message: String,
    /// Frequency in Hz
    pub frequency: f32,
    /// Time offset in seconds
    pub time_offset: f32,
    /// Sync quality metric
    pub sync_power: f32,
    /// SNR estimate (calculated from sync power)
    pub snr_db: i32,
    /// LDPC iterations required for decode
    pub ldpc_iterations: usize,
    /// LLR scaling factor that worked
    pub llr_scale: f32,
    /// Number of symbols used for demodulation (1, 2, or 3)
    pub nsym: usize,
    /// Tone sequence (79 tones, values 0-7) for signal subtraction
    pub tones: [u8; 79],
}

/// Internal struct to track decode results with candidate ordering
#[derive(Debug, Clone)]
struct DecodeResult {
    candidate_idx: usize,
    message: DecodedMessage,
}

/// Configuration for the FT8 decoder
#[derive(Debug, Clone)]
pub struct DecoderConfig {
    /// Minimum frequency to search (Hz)
    pub freq_min: f32,
    /// Maximum frequency to search (Hz)
    pub freq_max: f32,
    /// Minimum sync threshold for candidate detection
    pub sync_threshold: f32,
    /// Maximum number of candidates to try
    pub max_candidates: usize,
    /// Number of candidates to actually decode (top N by sync power)
    pub decode_top_n: usize,
    /// Minimum SNR threshold in dB (rejects weak false positives)
    pub min_snr_db: i32,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            freq_min: 100.0,
            freq_max: 3000.0,
            sync_threshold: 0.5,
            max_candidates: 100,
            decode_top_n: 50, // Increased from 30 to ensure weak signals aren't skipped in busy recordings
            min_snr_db: -18,  // Allow decoding down to -18 dB (WSJT-X typical minimum)
        }
    }
}

/// Decode all FT8 signals in a recording, calling the callback for each valid message found.
///
/// This follows the WSJT-X pattern: messages are reported immediately as found, not batched.
/// Duplicate messages (same text from same candidate) are automatically filtered.
///
/// The callback can return `false` to stop decoding early (e.g., after finding expected signals).
///
/// # Arguments
///
/// * `signal` - 15-second audio recording at 12 kHz sample rate
/// * `config` - Decoder configuration
/// * `callback` - Called immediately for each decoded message. Returns `true` to continue, `false` to stop.
///
/// # Returns
///
/// Total number of unique messages decoded
pub fn decode_ft8<F>(signal: &[f32], config: &DecoderConfig, mut callback: F) -> Result<usize, &'static str>
where
    F: FnMut(DecodedMessage) -> bool,
{
    // Coarse sync to find candidates
    let candidates = sync::coarse_sync(
        signal,
        config.freq_min,
        config.freq_max,
        config.sync_threshold,
        config.max_candidates,
    ).map_err(|_| "Coarse sync failed")?;

    if candidates.is_empty() {
        return Ok(0);
    }

    // LLR scaling factors to try (optimized order - most common values first)
    // Expanded range to help decode weaker signals
    let scaling_factors = [1.0, 1.5, 0.75, 2.0, 0.5, 1.25, 0.9, 1.1, 1.3, 1.7, 2.5, 3.0, 4.0, 5.0, 0.6, 0.8];
    let nsym_values = [1, 2, 3];

    // Process all candidates in parallel, collecting successful decodes
    let min_snr_threshold = config.min_snr_db;
    let decode_results: Vec<DecodeResult> = candidates
        .iter()
        .take(config.decode_top_n)
        .enumerate()
        .par_bridge()
        .filter_map(|(candidate_idx, candidate)| {
            // Fine sync on this candidate
            let refined = sync::fine_sync(signal, candidate).ok()?;

            // Try multi-pass decoding (different nsym and LLR scales)
            for &nsym in &nsym_values {
                let mut llr = vec![0.0f32; 174];
                let mut s8 = [[0.0f32; 79]; 8];

                // Extract symbols - try the new function with powers first, fall back to old on error
                let extract_ok = if let Ok(()) = sync::extract_symbols_with_powers(signal, &refined, nsym, &mut llr, &mut s8) {
                    true
                } else {
                    // Fall back to original function if new one fails
                    sync::extract_symbols(signal, &refined, nsym, &mut llr).is_ok()
                };

                if !extract_ok {
                    continue;
                }

                for &scale in &scaling_factors {
                    let mut scaled_llr = llr.clone();
                    for v in scaled_llr.iter_mut() {
                        *v *= scale;
                    }

                    // Use hybrid BP/OSD decoder matching WSJT-X strategy:
                    // - BP for 30 iterations with snapshots at iterations 1, 2, 3
                    // - If BP fails, try OSD order 2 with each saved snapshot
                    // - Multiple OSD attempts explore different solution space regions
                    if let Some((decoded_bits, iters)) = ldpc::decode_hybrid(&scaled_llr) {
                        // Re-encode the corrected message to get tones for signal subtraction
                        // (following WSJT-X: use LDPC-corrected tones, not original noisy demodulation)
                        let mut re_encoded_codeword = bitvec![u8, Msb0; 0; 174];
                        ldpc::encode(&decoded_bits, &mut re_encoded_codeword);
                        let mut tones = [0u8; 79];
                        if symbol::map(&re_encoded_codeword, &mut tones).is_err() {
                            continue; // Skip if tone mapping fails
                        }

                        let info_bits: BitVec<u8, Msb0> = decoded_bits.iter().take(77).collect();

                        if let Ok(message) = crate::decode(&info_bits, None) {
                            if !message.is_empty() {
                                // Calculate SNR using WSJT-X algorithm if we have s8 powers
                                // Pass baseline noise for improved SNR estimation
                                let snr_db = if s8[0][0] != 0.0 {
                                    sync::calculate_snr(&s8, &tones, Some(refined.baseline_noise))
                                } else {
                                    // Fallback for old extract_symbols path
                                    if refined.sync_power > 0.001 {
                                        let snr = (refined.sync_power.log10() * 10.0 - 27.0) as i32;
                                        snr.max(-24).min(30)
                                    } else {
                                        -24
                                    }
                                };

                                // Filter out weak decodes that are likely false positives
                                if snr_db < min_snr_threshold {
                                    continue; // Skip this decode, try next nsym/scale combination
                                }

                                // Return the first successful decode for this candidate
                                return Some(DecodeResult {
                                    candidate_idx,
                                    message: DecodedMessage {
                                        message,
                                        frequency: refined.frequency,
                                        time_offset: refined.time_offset,
                                        sync_power: refined.sync_power,
                                        snr_db,
                                        ldpc_iterations: iters,
                                        llr_scale: scale,
                                        nsym,
                                        tones,
                                    },
                                });
                            }
                        }
                    }
                }
            }

            None
        })
        .collect();

    // Sort by candidate index to maintain deterministic ordering
    let mut sorted_results = decode_results;
    sorted_results.sort_by_key(|r| r.candidate_idx);

    // Apply deduplication and call callbacks sequentially
    // Track (message, frequency, time) to detect duplicates
    let mut decoded_signals: Vec<(String, f32, f32)> = Vec::new();
    let mut decode_count = 0;

    for result in sorted_results {
        let message_text = &result.message.message;
        let freq = result.message.frequency;
        let time = result.message.time_offset;

        // Check for duplicate: same message within 10 Hz and 0.5s
        let is_duplicate = decoded_signals.iter().any(|(msg, f, t)| {
            msg == message_text && (freq - f).abs() < 10.0 && (time - t).abs() < 0.5
        });

        if !is_duplicate {
            decoded_signals.push((message_text.clone(), freq, time));
            decode_count += 1;

            // Report immediately via callback
            let should_continue = callback(result.message);

            // Stop decoding if callback returns false
            if !should_continue {
                return Ok(decode_count);
            }
        }
    }

    Ok(decode_count)
}

/// Decode all FT8 signals with multi-pass subtraction (like WSJT-X)
///
/// Performs multiple decode passes, subtracting decoded signals between passes
/// to reveal weaker signals that were masked by stronger ones.
///
/// # Arguments
///
/// * `signal` - 15-second audio recording at 12 kHz sample rate
/// * `config` - Decoder configuration
/// * `max_passes` - Maximum number of decode passes (typically 2-3)
/// * `callback` - Called immediately for each decoded message. Returns `true` to continue, `false` to stop.
///
/// # Returns
///
/// Total number of unique messages decoded across all passes
pub fn decode_ft8_multipass<F>(
    signal: &[f32],
    config: &DecoderConfig,
    max_passes: usize,
    mut callback: F,
) -> Result<usize, &'static str>
where
    F: FnMut(DecodedMessage) -> bool,
{
    let mut working_signal = signal.to_vec();
    let mut total_decodes = 0;
    let mut all_decoded_messages: Vec<String> = Vec::new();

    for pass_num in 0..max_passes {
        eprintln!("\n=== Pass {} ===", pass_num + 1);

        // Keep same config for all passes to avoid false positives from subtraction artifacts
        // (Lowering sync threshold makes it easier to find spurious peaks in residuals)
        let pass_config = config.clone();

        let mut pass_decodes = Vec::new();

        // Decode signals in current audio
        decode_ft8(&working_signal, &pass_config, |msg| {
            // Only report new messages (deduplication)
            if !all_decoded_messages.contains(&msg.message) {
                all_decoded_messages.push(msg.message.clone());
                pass_decodes.push(msg.clone());

                // Report to user
                let should_continue = callback(msg);
                if !should_continue {
                    return false;
                }
            }
            true
        })?;

        let pass_count = pass_decodes.len();
        total_decodes += pass_count;
        eprintln!("Pass {} decoded: {} new messages", pass_num + 1, pass_count);

        // Stop if no new signals found
        if pass_count == 0 {
            eprintln!("No new signals found, stopping");
            break;
        }

        // Subtract decoded signals (if not last pass)
        if pass_num < max_passes - 1 {
            eprintln!("Subtracting {} signals from audio...", pass_count);
            for decoded in &pass_decodes {
                if let Err(e) = crate::subtract::subtract_ft8_signal(
                    &mut working_signal,
                    &decoded.tones,
                    decoded.frequency,
                    decoded.time_offset,
                ) {
                    eprintln!("Warning: Signal subtraction failed: {}", e);
                }
            }
        }
    }

    eprintln!("\n=== Total: {} unique messages decoded ===\n", total_decodes);
    Ok(total_decodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_config_default() {
        let config = DecoderConfig::default();
        assert_eq!(config.freq_min, 100.0);
        assert_eq!(config.freq_max, 3000.0);
        assert!(config.sync_threshold > 0.0);
    }
}
