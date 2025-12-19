//! Multi-signal FT8 decoder
//!
//! Implements the complete FT8 decode pipeline for processing recordings with multiple signals.
//! Follows WSJT-X architecture: scans for candidates, decodes each, reports immediately via callback.

use crate::{ldpc, symbol, sync};
use bitvec::prelude::*;
use rayon::prelude::*;
use std::time::Instant;
use tracing::{debug, info, warn};

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
    /// Enable AP (a priori) decoding for weak signals
    pub enable_ap: bool,
    /// User's callsign for AP decoding (e.g., "K1BZM")
    pub mycall: Option<String>,
    /// DX station's callsign for AP decoding (e.g., "EA3GP")
    pub hiscall: Option<String>,
    /// Maximum decode passes (1 = single pass, >1 = multipass with signal subtraction)
    pub max_passes: usize,
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            freq_min: 100.0,
            freq_max: 3000.0,
            sync_threshold: 0.5,
            max_candidates: 1000, // Match WSJT-X MAXPRECAND (dual search generates more candidates)
            decode_top_n: 100, // Dual search generates ~2x candidates, need higher limit
            min_snr_db: -25,  // Allow decoding down to -25 dB for weak OSD signals
            enable_ap: true,  // AP enabled by default (Type 1 CQ pattern works without callsigns)
            mycall: None,     // Optional: configure for additional AP types (2-6)
            hiscall: None,    // Optional: configure for additional AP types (2-6)
            max_passes: 3,    // Multipass with subtraction (like WSJT-X)
        }
    }
}

/// Decode all FT8 signals in a recording, calling the callback for each valid message found.
///
/// This follows the WSJT-X pattern: messages are reported immediately as found, not batched.
/// Duplicate messages (same text from same candidate) are automatically filtered.
///
/// When `config.max_passes > 1`, performs multi-pass decoding with signal subtraction
/// to reveal weaker signals masked by stronger ones.
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
    if config.max_passes <= 1 {
        // Single pass - direct decode
        return decode_ft8_single_pass(signal, config, callback);
    }

    // Multi-pass decoding with signal subtraction
    let mut working_signal = signal.to_vec();
    let mut total_decodes = 0;
    let mut all_decoded_messages: Vec<String> = Vec::new();

    for pass_num in 0..config.max_passes {
        let pass_start = Instant::now();
        info!(pass = pass_num + 1, "Starting decode pass");

        let mut pass_decodes = Vec::new();

        // Decode signals in current audio
        decode_ft8_single_pass(&working_signal, config, |msg| {
            // Only report new messages (deduplication across passes)
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
        info!(pass = pass_num + 1, decoded = pass_count, elapsed_secs = pass_start.elapsed().as_secs_f64(), "Pass complete");

        // Stop if no new signals found
        if pass_count == 0 {
            debug!("No new signals found, stopping multipass");
            break;
        }

        // Subtract decoded signals (if not last pass)
        if pass_num < config.max_passes - 1 {
            debug!(count = pass_count, "Subtracting decoded signals from audio");
            for decoded in &pass_decodes {
                if let Err(e) = crate::sync::subtract_ft8_signal(
                    &mut working_signal,
                    &decoded.tones,
                    decoded.frequency,
                    decoded.time_offset,
                ) {
                    warn!(error = %e, "Signal subtraction failed");
                }
            }
        }
    }

    info!(total = total_decodes, "Multipass decode complete");
    Ok(total_decodes)
}

/// Single-pass decode (internal helper)
fn decode_ft8_single_pass<F>(signal: &[f32], config: &DecoderConfig, mut callback: F) -> Result<usize, &'static str>
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

    let num_candidates = candidates.len().min(config.decode_top_n);
    debug!(processing = num_candidates, found = candidates.len(), "Processing candidates");

    // LLR scaling factors to try (optimized order - most common values first)
    let scaling_factors = [1.0, 1.5, 0.75, 2.0, 0.5];

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

            // Try phase-based frequency refinement
            // This improves accuracy from 0.2 Hz to <0.05 Hz by measuring phase progression
            let candidate_to_decode = if let Ok(refined_freq) = sync::estimate_frequency_from_phase(signal, &refined) {
                let freq_correction = refined_freq - refined.frequency;
                // Only use refinement if correction is reasonable and significant
                if freq_correction.abs() < 1.0 && freq_correction.abs() > 0.01 {
                    let mut refined_candidate = refined.clone();
                    refined_candidate.frequency = refined_freq;
                    refined_candidate
                } else {
                    refined.clone()
                }
            } else {
                refined.clone()
            };

            // Extract ALL 4 LLR arrays in one pass (with independent normalization)
            // WSJT-X uses 4 separate passes: llra (nsym=1 diff), llrb (nsym=2), llrc (nsym=3), llrd (nsym=1 ratio)
            let mut llra = vec![0.0f32; 174];
            let mut llrb = vec![0.0f32; 174];
            let mut llrc = vec![0.0f32; 174];
            let mut llrd = vec![0.0f32; 174];
            let mut s8 = [[0.0f32; 79]; 8];

            let nsync = match sync::extract_symbols_all_llr(
                signal, &candidate_to_decode, &mut llra, &mut llrb, &mut llrc, &mut llrd, &mut s8
            ) {
                Ok(n) => n,
                Err(_) => return None,
            };

            // WSJT-X rejection filter #1: nsync must be > 6 (at least 7/21 Costas tones correct)
            if nsync <= 6 {
                return None;
            }

            // Try LLR methods with multiple scales
            // Only use nsym=1 methods for performance - nsym=2/3 rarely help
            let llr_methods: [(&str, &[f32], usize); 2] = [
                ("nsym1_ratio", &llrd[..], 1),  // Most reliable
                ("nsym1_diff", &llra[..], 1),   // Second best
            ];

            for &(method_name, llr, nsym) in &llr_methods {
                for &scale in &scaling_factors {
                    let mut scaled_llr: Vec<f32> = llr.to_vec();
                    for v in scaled_llr.iter_mut() {
                        *v *= scale;
                    }

                    // Progressive decoding strategy:
                    // 1. Try BP-only first - fast, minimal false positives
                    // 2. If BP fails, try OSD based on candidate rank:
                    //    - Top 10: BpOsdHybrid (accumulated LLR snapshots + ndeep 3-4)
                    //    - Top 30: BpOsdUncoupled (order 1 only, 91 patterns)
                    //    - Rest: no OSD (rely on BP only)
                    // OSD is pre-filtered by nharderrors > 50 check inside decode_hybrid
                    let decode_result = ldpc::decode_hybrid(&scaled_llr, ldpc::DecodeDepth::BpOnly)
                        .or_else(|| {
                            if candidate_idx < 10 {
                                // Top 10 candidates: try thorough OSD with accumulated snapshots
                                ldpc::decode_hybrid(&scaled_llr, ldpc::DecodeDepth::BpOsdHybrid)
                            } else if candidate_idx < 30 {
                                // Candidates 10-29: try fast OSD (order 1 only)
                                ldpc::decode_hybrid(&scaled_llr, ldpc::DecodeDepth::BpOsdUncoupled)
                            } else {
                                None
                            }
                        });

                    if let Some((decoded_bits, iters, nharderrors)) = decode_result {
                        // WSJT-X rejection filter #2: nharderrors threshold
                        // Note: nharderrors is parity check violations, not bit errors.
                        // For weak signals decoded via OSD, parity violations can be 40-50.
                        // We use a higher threshold (50) to allow OSD-decoded signals through.
                        if nharderrors > 50 {
                            continue;  // Reject candidates with too many parity violations
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

            // ===== AP (A Priori) Decoding Passes =====
            // If normal decoding failed and AP is enabled, try AP-assisted decoding
            // This extends decodable BER from ~10% (pure LDPC) to ~15-20% (AP + LDPC)
            if config.enable_ap {
                use crate::ap::{ApDecoder, ApType};

                // Create AP decoder from configuration
                let ap_decoder = ApDecoder::new(
                    config.mycall.clone(),
                    config.hiscall.clone(),
                );

                // Try each AP type (1-6) in order of likelihood
                let ap_types = [
                    ApType::CqAny,              // Type 1: CQ ??? ???
                    ApType::MyCallAny,          // Type 2: MYCALL ??? ???
                    ApType::MyCallDxCallAny,    // Type 3: MYCALL DXCALL ???
                    ApType::MyCallDxCallRrr,    // Type 4: MYCALL DXCALL RRR
                    ApType::MyCallDxCall73,     // Type 5: MYCALL DXCALL 73
                    ApType::MyCallDxCallRr73,   // Type 6: MYCALL DXCALL RR73
                ];

                for ap_type in &ap_types {
                    // Compute LLR magnitude for AP hints (max absolute value * 1.01)
                    let llr_magnitude = llra.iter()
                        .map(|x| x.abs())
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap_or(10.0) * 1.01;

                    // Generate AP hints for this type
                    let ap_hints = match ap_decoder.generate_ap_hints(*ap_type, llr_magnitude) {
                        Some(hints) => hints,
                        None => continue, // Skip if we can't generate hints for this type
                    };

                    let (apmask, llr_hints) = ap_hints;

                    // Try AP decoding with each of the 4 LLR methods
                    for &(method_name, base_llr, nsym) in &llr_methods {
                        // Apply AP hints to the LLRs
                        let mut llr_with_ap = base_llr.to_vec();
                        for i in 0..174 {
                            if apmask[i] {
                                llr_with_ap[i] = llr_hints[i];
                            }
                        }

                        // Try decoding with AP mask - BP only (fast)
                        let decode_result = ldpc::decode_hybrid_with_ap(
                            &llr_with_ap,
                            Some(&apmask),
                            ldpc::DecodeDepth::BpOnly
                        );

                        if let Some((decoded_bits, iters, nharderrors)) = decode_result {
                            // Same rejection filters as normal decoding
                            if nharderrors > 36 {
                                continue;
                            }

                            let mut re_encoded_codeword = bitvec![u8, Msb0; 0; 174];
                            ldpc::encode(&decoded_bits, &mut re_encoded_codeword);
                            let mut tones = [0u8; 79];
                            if symbol::map(&re_encoded_codeword, &mut tones).is_err() {
                                continue;
                            }

                            if tones.iter().all(|&t| t == 0) {
                                continue;
                            }

                            let info_bits: BitVec<u8, Msb0> = decoded_bits.iter().take(77).collect();

                            if let Ok(message) = crate::decode(&info_bits, None) {
                                if !message.is_empty() {
                                    let tokens: Vec<&str> = message.split_whitespace().collect();
                                    let is_valid_message = if tokens.len() >= 2 {
                                        crate::message::is_valid_callsign(tokens[0]) &&
                                        crate::message::is_valid_callsign(tokens[1])
                                    } else {
                                        tokens.first().map_or(false, |t| crate::message::is_valid_callsign(t))
                                    };

                                    if !is_valid_message {
                                        continue;
                                    }

                                    let snr_db = if s8[0][0] != 0.0 {
                                        sync::calculate_snr(&s8, &tones, Some(candidate_to_decode.baseline_noise))
                                    } else {
                                        if candidate_to_decode.sync_power > 0.001 {
                                            let snr = (candidate_to_decode.sync_power.log10() * 10.0 - 27.0) as i32;
                                            snr.max(-24).min(30)
                                        } else {
                                            -24
                                        }
                                    };

                                    if nsync <= 10 && snr_db < -24 {
                                        continue;
                                    }

                                    if snr_db < min_snr_threshold {
                                        continue;
                                    }

                                    // AP decode succeeded!
                                    return Some(DecodeResult {
                                        candidate_idx,
                                        message: DecodedMessage {
                                            message,
                                            frequency: candidate_to_decode.frequency,
                                            time_offset: candidate_to_decode.time_offset,
                                            sync_power: candidate_to_decode.sync_power,
                                            snr_db,
                                            ldpc_iterations: iters,
                                            llr_scale: 1.0, // AP uses unscaled LLRs
                                            nsym,
                                            tones,
                                        },
                                    });
                                }
                            }
                        }
                    }
                }
            }
            // ===== End AP Passes =====

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
