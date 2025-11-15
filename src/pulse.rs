//! FT8 Pulse Shaping and Waveform Generation
//!
//! This module generates FT8 audio waveforms using Gaussian Frequency Shift Keying (GFSK)
//! with smooth transitions between symbol tones.
//!
//! **Key Parameters**:
//! - Symbol duration: 0.16 seconds (1920 samples at 12000 Hz)
//! - Bandwidth-time product (BT): 2.0
//! - Tone spacing: 6.25 Hz
//! - Total bandwidth: ~50 Hz (8 tones)
//!
//! **Process**:
//! 1. Apply GFSK pulse shaping to smooth frequency transitions
//! 2. Generate phase-continuous waveform
//! 3. Apply envelope shaping to first and last symbols (cosine ramp)

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;
use alloc::string::String;

/// FT8 sample rate in Hz
pub const SAMPLE_RATE: f32 = 12000.0;

/// Samples per symbol at 12 kHz
pub const NSPS: usize = 1920;

/// Bandwidth-time product for GFSK
pub const BT: f32 = 2.0;

/// Tone spacing in Hz
pub const TONE_SPACING: f32 = 6.25;

/// Compute the GFSK pulse shape at time t
///
/// This implements the Gaussian frequency-shift keying pulse:
/// `pulse(t) = 0.5 * (erf(c*b*(t+0.5)) - erf(c*b*(t-0.5)))`
///
/// where `c = π * sqrt(2 / ln(2))` and `b` is the bandwidth-time product.
///
/// # Arguments
/// * `bt` - Bandwidth-time product (typically 2.0 for FT8)
/// * `t` - Time normalized by symbol period (0 at symbol center)
///
/// # Returns
/// * Pulse amplitude at time t
fn gfsk_pulse(bt: f32, t: f32) -> f32 {
    use core::f32::consts::PI;

    let c = PI * libm::sqrtf(2.0 / libm::logf(2.0));
    let arg1 = c * bt * (t + 0.5);
    let arg2 = c * bt * (t - 0.5);

    0.5 * (libm::erff(arg1) - libm::erff(arg2))
}

/// Generate an FT8 waveform from 79 symbols
///
/// Creates a phase-continuous audio waveform using GFSK pulse shaping.
/// The waveform includes smooth transitions between tones and envelope
/// shaping on the first and last symbols.
///
/// # Arguments
/// * `symbols` - Array of 79 FT8 symbols (tones 0-7)
/// * `f0` - Base frequency in Hz (typically 1000-2000 Hz)
/// * `sample_rate` - Sample rate in Hz (typically 12000)
/// * `nsps` - Samples per symbol (typically 1920 at 12 kHz)
///
/// # Returns
/// * `Result<Vec<f32>, String>` - Audio samples or error message
///
/// # Example
/// ```no_run
/// use rustyft8::pulse;
///
/// let symbols = [0u8; 79]; // 79 FT8 symbols
/// let waveform = pulse::generate_waveform(&symbols, 1500.0, 12000.0, 1920)?;
/// // waveform contains 151,680 samples (79 symbols * 1920 samples/symbol)
/// # Ok::<(), String>(())
/// ```
pub fn generate_waveform(
    symbols: &[u8; 79],
    f0: f32,
    sample_rate: f32,
    nsps: usize,
) -> Result<Vec<f32>, String> {
    use core::f32::consts::PI;

    let nsym = symbols.len();
    let bt = BT;
    let twopi = 2.0 * PI;
    let dt = 1.0 / sample_rate;
    let hmod = 1.0; // Modulation index

    // Compute the frequency-smoothing pulse (extends 1.5 symbols on each side)
    let pulse_len = 3 * nsps;
    let mut pulse = Vec::with_capacity(pulse_len);
    for i in 0..pulse_len {
        let tt = (i as f32 - 1.5 * nsps as f32) / nsps as f32;
        pulse.push(gfsk_pulse(bt, tt));
    }

    // Compute the smoothed frequency waveform
    // Length = (nsym+2)*nsps samples (includes dummy symbols at start/end)
    let dphi_len = (nsym + 2) * nsps;
    let mut dphi = vec![0.0f32; dphi_len];

    let dphi_peak = twopi * hmod / nsps as f32;

    // Apply pulse shaping to each symbol
    for j in 0..nsym {
        let ib = j * nsps;

        for (k, &p) in pulse.iter().enumerate() {
            if ib + k < dphi_len {
                dphi[ib + k] += dphi_peak * p * symbols[j] as f32;
            }
        }
    }

    // Add dummy symbols at beginning and end
    // First dummy: extends left from first symbol
    for k in 0..(2 * nsps).min(dphi_len) {
        if nsps + k < pulse_len {
            dphi[k] += dphi_peak * symbols[0] as f32 * pulse[nsps + k];
        }
    }

    // Last dummy: extends right from last symbol
    let last_start = nsym * nsps;
    for k in 0..(2 * nsps).min(dphi_len - last_start) {
        if k < pulse_len {
            dphi[last_start + k] += dphi_peak * symbols[nsym - 1] as f32 * pulse[k];
        }
    }

    // Shift frequency up by f0
    let f0_dphi = twopi * f0 * dt;
    for d in dphi.iter_mut() {
        *d += f0_dphi;
    }

    // Generate the waveform (skip first dummy symbol)
    let nwave = nsym * nsps;
    let mut wave = Vec::with_capacity(nwave);
    let mut phi = 0.0f32;

    for j in nsps..(nsps + nwave) {
        wave.push(libm::sinf(phi));
        phi = (phi + dphi[j]) % twopi;
    }

    // Apply envelope shaping to first and last symbols (cosine ramp)
    let nramp = (nsps as f32 / 8.0) as usize;

    // Ramp up at start
    for i in 0..nramp {
        let envelope = (1.0 - libm::cosf(twopi * i as f32 / (2.0 * nramp as f32))) / 2.0;
        wave[i] *= envelope;
    }

    // Ramp down at end
    let k1 = nsym * nsps - nramp;
    for i in 0..nramp {
        let envelope = (1.0 + libm::cosf(twopi * i as f32 / (2.0 * nramp as f32))) / 2.0;
        if k1 + i < wave.len() {
            wave[k1 + i] *= envelope;
        }
    }

    Ok(wave)
}

/// Generate a complex (I/Q) FT8 waveform from 79 symbols
///
/// Similar to `generate_waveform` but produces complex samples for SDR applications.
/// Uses a lookup table for efficient sin/cos computation.
///
/// # Arguments
/// * `symbols` - Array of 79 FT8 symbols (tones 0-7)
/// * `f0` - Base frequency in Hz
/// * `sample_rate` - Sample rate in Hz
/// * `nsps` - Samples per symbol
///
/// # Returns
/// * `Result<Vec<(f32, f32)>, String>` - Complex samples (I, Q) or error
///
/// # Example
/// ```no_run
/// use rustyft8::pulse;
///
/// let symbols = [0u8; 79];
/// let waveform = pulse::generate_complex_waveform(&symbols, 1500.0, 12000.0, 1920)?;
/// // Each sample is (I, Q) tuple
/// # Ok::<(), String>(())
/// ```
pub fn generate_complex_waveform(
    symbols: &[u8; 79],
    f0: f32,
    sample_rate: f32,
    nsps: usize,
) -> Result<Vec<(f32, f32)>, String> {
    use core::f32::consts::PI;

    let nsym = symbols.len();
    let bt = BT;
    let twopi = 2.0 * PI;
    let dt = 1.0 / sample_rate;
    let hmod = 1.0;

    // Compute pulse
    let pulse_len = 3 * nsps;
    let mut pulse = Vec::with_capacity(pulse_len);
    for i in 0..pulse_len {
        let tt = (i as f32 - 1.5 * nsps as f32) / nsps as f32;
        pulse.push(gfsk_pulse(bt, tt));
    }

    // Compute dphi
    let dphi_len = (nsym + 2) * nsps;
    let mut dphi = vec![0.0f32; dphi_len];
    let dphi_peak = twopi * hmod / nsps as f32;

    for j in 0..nsym {
        let ib = j * nsps;
        for (k, &p) in pulse.iter().enumerate() {
            if ib + k < dphi_len {
                dphi[ib + k] += dphi_peak * p * symbols[j] as f32;
            }
        }
    }

    // Dummy symbols
    for k in 0..(2 * nsps).min(dphi_len) {
        if nsps + k < pulse_len {
            dphi[k] += dphi_peak * symbols[0] as f32 * pulse[nsps + k];
        }
    }
    let last_start = nsym * nsps;
    for k in 0..(2 * nsps).min(dphi_len - last_start) {
        if k < pulse_len {
            dphi[last_start + k] += dphi_peak * symbols[nsym - 1] as f32 * pulse[k];
        }
    }

    // Shift by f0
    let f0_dphi = twopi * f0 * dt;
    for d in dphi.iter_mut() {
        *d += f0_dphi;
    }

    // Generate complex waveform
    let nwave = nsym * nsps;
    let mut cwave = Vec::with_capacity(nwave);
    let mut phi = 0.0f32;

    for j in nsps..(nsps + nwave) {
        let i_sample = libm::cosf(phi);
        let q_sample = libm::sinf(phi);
        cwave.push((i_sample, q_sample));
        phi = (phi + dphi[j]) % twopi;
    }

    // Apply envelope shaping
    let nramp = (nsps as f32 / 8.0) as usize;

    for i in 0..nramp {
        let envelope = (1.0 - libm::cosf(twopi * i as f32 / (2.0 * nramp as f32))) / 2.0;
        cwave[i].0 *= envelope;
        cwave[i].1 *= envelope;
    }

    let k1 = nsym * nsps - nramp;
    for i in 0..nramp {
        let envelope = (1.0 + libm::cosf(twopi * i as f32 / (2.0 * nramp as f32))) / 2.0;
        if k1 + i < cwave.len() {
            cwave[k1 + i].0 *= envelope;
            cwave[k1 + i].1 *= envelope;
        }
    }

    Ok(cwave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gfsk_pulse_properties() {
        // Test that pulse integrates to approximately 1
        let bt = 2.0;
        let mut sum = 0.0;
        let n = 1000;
        for i in 0..n {
            let t = (i as f32 / n as f32) - 0.5;
            sum += gfsk_pulse(bt, t);
        }
        sum /= n as f32;

        // Should be close to 1.0
        assert!((sum - 1.0).abs() < 0.1, "Pulse integral should be ~1.0, got {}", sum);
    }

    #[test]
    fn test_gfsk_pulse_symmetry() {
        // Pulse should be symmetric around t=0
        let bt = 2.0;
        for i in 1..10 {
            let t = i as f32 * 0.1;
            let p_pos = gfsk_pulse(bt, t);
            let p_neg = gfsk_pulse(bt, -t);
            assert!((p_pos - p_neg).abs() < 1e-6, "Pulse should be symmetric");
        }
    }

    #[test]
    fn test_generate_waveform_length() {
        let symbols = [0u8; 79];
        let result = generate_waveform(&symbols, 1500.0, 12000.0, 1920);

        assert!(result.is_ok());
        let wave = result.unwrap();

        // Should be 79 symbols * 1920 samples/symbol = 151,680 samples
        assert_eq!(wave.len(), 79 * 1920);
    }

    #[test]
    fn test_generate_waveform_all_zeros() {
        // All zero tones should produce a constant frequency (just f0)
        let symbols = [0u8; 79];
        let result = generate_waveform(&symbols, 1500.0, 12000.0, 1920);

        assert!(result.is_ok());
        let wave = result.unwrap();

        // Check that wave is bounded [-1, 1]
        for &sample in &wave {
            assert!(sample >= -1.0 && sample <= 1.0, "Sample out of range: {}", sample);
        }
    }

    #[test]
    fn test_generate_waveform_envelope() {
        let symbols = [4u8; 79]; // All mid-tone
        let wave = generate_waveform(&symbols, 1500.0, 12000.0, 1920).unwrap();

        // First sample should be near zero (envelope starts at 0)
        assert!(wave[0].abs() < 0.1, "First sample should be small due to envelope, got {}", wave[0]);

        // Last sample should be near zero (envelope ends at 0)
        let last_idx = wave.len() - 1;
        assert!(wave[last_idx].abs() < 0.1, "Last sample should be small due to envelope, got {}", wave[last_idx]);

        // Check that envelope ramps up in the first symbol
        // Sample at 1/4 of first symbol should be less than sample at 3/4
        let quarter = 1920 / 4;
        let three_quarter = 3 * 1920 / 4;
        assert!(wave[quarter].abs() < wave[three_quarter].abs(),
                "Envelope should ramp up: {} < {}", wave[quarter].abs(), wave[three_quarter].abs());
    }

    #[test]
    fn test_generate_complex_waveform_length() {
        let symbols = [0u8; 79];
        let result = generate_complex_waveform(&symbols, 1500.0, 12000.0, 1920);

        assert!(result.is_ok());
        let cwave = result.unwrap();
        assert_eq!(cwave.len(), 79 * 1920);
    }

    #[test]
    fn test_generate_complex_waveform_unit_magnitude() {
        let symbols = [4u8; 79];
        let cwave = generate_complex_waveform(&symbols, 1500.0, 12000.0, 1920).unwrap();

        // Skip envelope regions and check middle samples
        let start = 1920; // After first symbol
        let end = cwave.len() - 1920; // Before last symbol

        for i in start..end.min(start + 100) {
            let (i_val, q_val) = cwave[i];
            let magnitude = libm::sqrtf(i_val * i_val + q_val * q_val);

            // Should be close to 1.0 (unit circle)
            assert!((magnitude - 1.0).abs() < 0.1,
                    "Complex magnitude should be ~1.0, got {} at sample {}", magnitude, i);
        }
    }

    #[test]
    fn test_different_tones_produce_different_waveforms() {
        let symbols_low = [0u8; 79];  // All tone 0
        let symbols_high = [7u8; 79]; // All tone 7

        let wave_low = generate_waveform(&symbols_low, 1500.0, 12000.0, 1920).unwrap();
        let wave_high = generate_waveform(&symbols_high, 1500.0, 12000.0, 1920).unwrap();

        // Waveforms should be different (different frequencies)
        let mut differences = 0;
        for i in 1920..2000 { // Check a segment in the middle
            if (wave_low[i] - wave_high[i]).abs() > 0.01 {
                differences += 1;
            }
        }

        assert!(differences > 50, "Different tones should produce different waveforms");
    }
}
