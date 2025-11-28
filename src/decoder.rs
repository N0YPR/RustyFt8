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
            max_candidates: 1000, // Match WSJT-X MAXPRECAND (dual search generates more candidates)
            decode_top_n: 100, // Dual search generates ~2x candidates, need higher limit
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
    // Disable nsym=2/3: creates more false positives than correct decodes
    // Testing showed: nsym=1 gives 8 correct + 1 false positive (11%)
    //                 nsym=1/2/3 gives 8 correct + 2 false positives (20%)
    // Need better phase tracking or LLR quality before nsym=2/3 is useful
    let nsym_values = [1];

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

            // Try phase-based frequency refinement first
            // This improves accuracy from 0.2 Hz to <0.05 Hz by measuring
            // phase progression of Costas arrays
            let mut candidates_to_try = vec![refined.clone()];

            if let Ok(refined_freq) = sync::estimate_frequency_from_phase(signal, &refined) {
                let freq_correction = refined_freq - refined.frequency;

                // Only use refinement if correction is reasonable and significant
                // Sanity: < 1 Hz (avoid wild corrections), > 0.01 Hz (worth re-extracting)
                if freq_correction.abs() < 1.0 && freq_correction.abs() > 0.01 {
                    let mut refined_candidate = refined.clone();
                    refined_candidate.frequency = refined_freq;

                    // Try refined candidate first (better frequency), then original
                    candidates_to_try = vec![refined_candidate, refined.clone()];

                    // Debug output (disabled by default)
                    let _debug_phase_refine = false;
                    if _debug_phase_refine {
                        eprintln!("PHASE_REFINE: freq_initial={:.1} Hz -> freq_refined={:.1} Hz (correction={:+.3} Hz)",
                                 refined.frequency, refined_freq, freq_correction);
                    }
                }
            }

            // Try multi-pass decoding with ALL 4 LLR methods (matching WSJT-X exactly)
            // WSJT-X uses 4 separate passes with independent normalization:
            // Pass 1: llra (nsym=1 difference), Pass 2: llrb (nsym=2), Pass 3: llrc (nsym=3), Pass 4: llrd (nsym=1 ratio)
            // Try all candidate frequencies (refined first if available, then original)
            for candidate_to_decode in &candidates_to_try {
                let mut llra = vec![0.0f32; 174];   // nsym=1 difference
                let mut llrb = vec![0.0f32; 174];   // nsym=2 difference
                let mut llrc = vec![0.0f32; 174];   // nsym=3 difference
                let mut llrd = vec![0.0f32; 174];   // nsym=1 ratio
                let mut s8 = [[0.0f32; 79]; 8];

                // Extract ALL 4 LLR arrays in one pass (with independent normalization)
                let nsync = match sync::extract_symbols_all_llr(
                    signal, candidate_to_decode, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8
                ) {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                // WSJT-X rejection filter #1: nsync must be > 6 (at least 7/21 Costas tones correct)
                // This filters out candidates where sync quality is too low
                if nsync <= 6 {
                    continue;  // Reject weak sync candidates
                }

                // Try all 4 LLR methods with multiple scales (matching WSJT-X 4-pass strategy)
                let llr_methods: [(&str, &[f32], usize); 4] = [
                    ("nsym1_diff", &llra[..], 1),   // Pass 1
                    ("nsym2_diff", &llrb[..], 2),   // Pass 2
                    ("nsym3_diff", &llrc[..], 3),   // Pass 3
                    ("nsym1_ratio", &llrd[..], 1),  // Pass 4
                ];

                for &(method_name, llr, nsym) in &llr_methods {
                    for &scale in &scaling_factors {
                    let mut scaled_llr: Vec<f32> = llr.to_vec();
                    for v in scaled_llr.iter_mut() {
                        *v *= scale;
                    }

                    // eprintln!("  LDPC_ATTEMPT: freq={:.1} Hz, dt={:.2}s, method={}, scale={:.1}, rank={}",
                    //          refined.frequency, refined.time_offset, method_name, scale, candidate_idx);

                    // Progressive decoding strategy (matching WSJT-X):
                    // 1. Try BP-only first (maxosd=-1) - fast, minimal false positives
                    // 2. If BP fails, try BP+OSD uncoupled (maxosd=0) - moderate aggression
                    // 3. For top 20 candidates, try full hybrid OSD (maxosd=2) - most aggressive
                    // This balances finding weak signals vs limiting false positives
                    let decode_result = ldpc::decode_hybrid(&scaled_llr, ldpc::DecodeDepth::BpOnly)
                        .or_else(|| ldpc::decode_hybrid(&scaled_llr, ldpc::DecodeDepth::BpOsdUncoupled))
                        .or_else(|| {
                            // Only use aggressive hybrid OSD for strongest candidates
                            // Limit to top 20 to minimize false positives from spurious candidates
                            if candidate_idx < 20 {
                                ldpc::decode_hybrid(&scaled_llr, ldpc::DecodeDepth::BpOsdHybrid)
                            } else {
                                None
                            }
                        });

                    if let Some((decoded_bits, iters, nharderrors)) = decode_result {
                        // WSJT-X rejection filter #2: nharderrors must be <= 36
                        // This filters out OSD false positives from extremely noisy candidates
                        if nharderrors > 36 {
                            continue;  // Reject candidates with too many initial hard errors
                        }

                        // Re-encode the corrected message to get tones for signal subtraction
                        // (following WSJT-X: use LDPC-corrected tones, not original noisy demodulation)
                        let mut re_encoded_codeword = bitvec![u8, Msb0; 0; 174];
                        ldpc::encode(&decoded_bits, &mut re_encoded_codeword);
                        let mut tones = [0u8; 79];
                        if symbol::map(&re_encoded_codeword, &mut tones).is_err() {
                            continue; // Skip if tone mapping fails
                        }

                        // WSJT-X rejection filter #3: all-zero codeword check
                        // OSD can sometimes produce all-zero codewords from noise
                        if tones.iter().all(|&t| t == 0) {
                            continue;  // Reject all-zero codewords
                        }

                        let info_bits: BitVec<u8, Msb0> = decoded_bits.iter().take(77).collect();

                        if let Ok(message) = crate::decode(&info_bits, None) {
                            // Debug: log LDPC decoder type and LLR method (disabled by default)
                            let _debug_ldpc = false;
                            if _debug_ldpc {
                                let decode_type = if iters == 0 {
                                    "OSD"
                                } else {
                                    "BP"
                                };
                                eprintln!("  LDPC: {} iters={}, method={}, freq={:.1} Hz, nsym={}, scale={:.1}",
                                         decode_type, iters, method_name, refined.frequency, nsym, scale);
                            }
                            if !message.is_empty() {
                                // Validate that the message contains valid callsigns
                                // This filters out OSD false positives (garbage decoded from noise)
                                let tokens: Vec<&str> = message.split_whitespace().collect();

                                // For standard messages, require ALL callsigns to be valid
                                // First 2 tokens are typically callsigns or "CQ"
                                let is_valid_message = if tokens.len() >= 2 {
                                    crate::message::is_valid_callsign(tokens[0]) &&
                                    crate::message::is_valid_callsign(tokens[1])
                                } else {
                                    // Short messages - require at least the first token to be valid
                                    tokens.first().map_or(false, |t| crate::message::is_valid_callsign(t))
                                };

                                if !is_valid_message {
                                    continue; // Skip messages with invalid callsigns
                                }

                                // Calculate SNR using WSJT-X algorithm if we have s8 powers
                                // Pass baseline noise for improved SNR estimation
                                let snr_db = if s8[0][0] != 0.0 {
                                    sync::calculate_snr(&s8, &tones, Some(candidate_to_decode.baseline_noise))
                                } else {
                                    // Fallback for old extract_symbols path
                                    if candidate_to_decode.sync_power > 0.001 {
                                        let snr = (candidate_to_decode.sync_power.log10() * 10.0 - 27.0) as i32;
                                        snr.max(-24).min(30)
                                    } else {
                                        -24
                                    }
                                };

                                // WSJT-X rejection filter #4: Combined sync + SNR check
                                // If sync quality is weak (nsync ≤ 10) AND SNR is very low (< -24 dB),
                                // this is likely a false positive (WSJT-X ft8b.f90 line 456-459)
                                if nsync <= 10 && snr_db < -24 {
                                    continue;  // Reject weak sync + very low SNR
                                }

                                // Filter out weak decodes that are likely false positives
                                if snr_db < min_snr_threshold {
                                    continue; // Skip this decode, try next nsym/scale combination
                                }

                                // Additional filtering for OSD decodes (iters==0)
                                // OSD can decode noise into valid messages, so be more strict
                                // DISABLED: This filters false positives but also stops Pass 3 from running
                                let _enable_osd_filter = false;
                                if _enable_osd_filter && iters == 0 && snr_db < -15 {
                                    // OSD decodes below -15 dB are likely false positives
                                    // especially in Pass 2+ after subtraction
                                    // This threshold still allows marginal OSD decodes (-15 to -12 dB)
                                    // but filters very weak ones that are probably noise
                                    continue;
                                }

                                // Return the first successful decode for this candidate
                                return Some(DecodeResult {
                                    candidate_idx,
                                    message: DecodedMessage {
                                        message,
                                        frequency: candidate_to_decode.frequency,
                                        time_offset: candidate_to_decode.time_offset,
                                        sync_power: candidate_to_decode.sync_power,
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
                }  // End for &scale loop
            }  // End for &(method_name, llr, nsym) loop (4 LLR methods)
            }  // End for candidate_to_decode loop

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
