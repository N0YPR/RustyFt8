//! Multi-signal FT8 decoder
//!
//! Implements the complete FT8 decode pipeline for processing recordings with multiple signals.
//! Follows WSJT-X architecture: scans for candidates, decodes each, reports immediately via callback.

use crate::{ldpc, sync};
use bitvec::prelude::*;

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
}

impl Default for DecoderConfig {
    fn default() -> Self {
        Self {
            freq_min: 100.0,
            freq_max: 3000.0,
            sync_threshold: 0.5,
            max_candidates: 100,
            decode_top_n: 50, // Decode top 50 candidates like WSJT-X
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

    // Track decoded messages for deduplication
    let mut decoded_messages: Vec<String> = Vec::new();
    let mut decode_count = 0;

    // LLR scaling factors to try (optimized order - most common values first)
    let scaling_factors = [1.0, 1.5, 0.75, 2.0, 0.5, 2.5, 3.0, 4.0, 5.0, 1.25];
    let nsym_values = [1, 2, 3];

    // Process candidates in order of sync quality
    'candidate_loop: for candidate in candidates.iter().take(config.decode_top_n) {
        // Fine sync on this candidate
        let refined = match sync::fine_sync(signal, candidate) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Try multi-pass decoding (different nsym and LLR scales)
        for &nsym in &nsym_values {
            let mut llr = vec![0.0f32; 174];
            if sync::extract_symbols(signal, &refined, nsym, &mut llr).is_err() {
                continue;
            }

            for &scale in &scaling_factors {
                let mut scaled_llr = llr.clone();
                for v in scaled_llr.iter_mut() {
                    *v *= scale;
                }

                if let Some((decoded_bits, iters)) = ldpc::decode(&scaled_llr, 100) {
                    let info_bits: BitVec<u8, Msb0> = decoded_bits.iter().take(77).collect();

                    if let Ok(message) = crate::decode(&info_bits, None) {
                        if !message.is_empty() {
                            // Check for duplicate
                            if !decoded_messages.contains(&message) {
                                decoded_messages.push(message.clone());
                                decode_count += 1;

                                // Estimate SNR from sync power (rough approximation)
                                let snr_db = (refined.sync_power.log10() * 10.0 - 30.0) as i32;

                                // Report immediately via callback
                                let should_continue = callback(DecodedMessage {
                                    message,
                                    frequency: refined.frequency,
                                    time_offset: refined.time_offset,
                                    sync_power: refined.sync_power,
                                    snr_db,
                                    ldpc_iterations: iters,
                                    llr_scale: scale,
                                    nsym,
                                });

                                // Stop decoding if callback returns false
                                if !should_continue {
                                    return Ok(decode_count);
                                }

                                // Move to next candidate after successful decode
                                continue 'candidate_loop;
                            }
                        }
                    }
                }
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
