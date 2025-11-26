///! FT8 signal synthesis for signal subtraction
///!
///! Synthesizes FT8 signals from tone sequences using GFSK modulation.
///! Used for multi-pass decoding to subtract decoded signals from the audio.

use super::{SAMPLE_RATE, NSPS};

const TWOPI: f32 = 2.0 * core::f32::consts::PI;
const TONE_SPACING: f32 = 6.25; // Hz

/// Gaussian pulse for GFSK frequency shaping
///
/// BT = 2.0 (bandwidth-time product)
/// Matches WSJT-X gen_ft8wave.f90
fn gfsk_pulse(bt: f32, t: f32) -> f32 {
    let pi = core::f32::consts::PI;
    let c = pi * (2.0 * bt / f32::sqrt(f32::ln(2.0)));
    let arg = c * t;
    f32::exp(-arg * arg)
}

/// Generate Gaussian-smoothed frequency pulse
///
/// Returns a pulse that spans 3 symbol periods for smooth frequency transitions
fn generate_pulse(nsps: usize, bt: f32) -> Vec<f32> {
    let mut pulse = vec![0.0; 3 * nsps];

    for i in 0..pulse.len() {
        let tt = (i as f32 - 1.5 * nsps as f32) / nsps as f32;
        pulse[i] = gfsk_pulse(bt, tt);
    }

    pulse
}

/// Synthesize FT8 signal from tone sequence
///
/// Generates a complex baseband signal using Gaussian-filtered FSK modulation.
/// The signal can then be mixed with the original audio for subtraction.
///
/// # Arguments
///
/// * `tones` - 79 tones (values 0-7)
/// * `f0` - Center frequency in Hz
/// * `output` - Complex output buffer (must be at least NMAX=180000 samples)
///
/// # Returns
///
/// Number of samples written
pub fn synthesize_ft8_signal(
    tones: &[u8; 79],
    f0: f32,
    output: &mut [(f32, f32)],
) -> Result<usize, String> {
    const NSYM: usize = 79;
    const BT: f32 = 2.0; // Gaussian bandwidth-time product
    const NMAX: usize = 15 * 12000; // Maximum signal length

    if output.len() < NMAX {
        return Err(format!("Output buffer too small: {} (need {})", output.len(), NMAX));
    }

    // Generate GFSK pulse (matches WSJT-X)
    let pulse = generate_pulse(NSPS, BT);

    // Compute smoothed frequency waveform (phase derivative)
    // Length = (nsym+2)*nsps to include dummy symbols at edges
    let dphi_len = (NSYM + 2) * NSPS;
    let mut dphi = vec![0.0f32; dphi_len];

    let dphi_peak = TWOPI / NSPS as f32; // Peak phase deviation

    // Add frequency contribution from each tone
    for j in 0..NSYM {
        let ib = j * NSPS;
        let ie = ib + 3 * NSPS - 1;

        if ie < dphi.len() {
            for (k, &p) in pulse.iter().enumerate() {
                let idx = ib + k;
                if idx < dphi.len() {
                    dphi[idx] += dphi_peak * p * tones[j] as f32;
                }
            }
        }
    }

    // Add dummy symbols at beginning and end with first/last tone values
    // This prevents discontinuities at signal edges
    for k in 0..2*NSPS {
        if k < dphi.len() {
            let pulse_idx = NSPS + k;
            if pulse_idx < pulse.len() {
                dphi[k] += dphi_peak * tones[0] as f32 * pulse[pulse_idx];
            }
        }
    }

    let dummy_start = NSYM * NSPS;
    for k in 0..2*NSPS {
        let idx = dummy_start + k;
        if idx < dphi.len() && k < pulse.len() {
            dphi[idx] += dphi_peak * tones[NSYM - 1] as f32 * pulse[k];
        }
    }

    // Shift frequency up to f0
    let dt = 1.0 / SAMPLE_RATE;
    for d in dphi.iter_mut() {
        *d += TWOPI * f0 * dt;
    }

    // Generate complex waveform from phase
    let nwave = NSYM * NSPS; // Don't include dummy symbols in output
    let mut phi = 0.0f32;

    for k in 0..nwave {
        let j = NSPS + k; // Skip first dummy symbol
        if j < dphi.len() {
            // Complex exponential: exp(j*phi)
            output[k] = (f32::cos(phi), f32::sin(phi));
            phi = (phi + dphi[j]) % TWOPI;
        }
    }

    // Apply envelope shaping to first and last symbols
    // Smooth ramp-up/down to prevent spectral splatter
    let nramp = (NSPS as f32 / 8.0).round() as usize;

    // Ramp up at start
    for i in 0..nramp {
        let env = (1.0 - f32::cos(TWOPI * i as f32 / (2.0 * nramp as f32))) / 2.0;
        output[i].0 *= env;
        output[i].1 *= env;
    }

    // Ramp down at end
    let k1 = nwave - nramp;
    for i in 0..nramp {
        let env = (1.0 + f32::cos(TWOPI * i as f32 / (2.0 * nramp as f32))) / 2.0;
        output[k1 + i].0 *= env;
        output[k1 + i].1 *= env;
    }

    Ok(nwave)
}

/// Subtract synthesized FT8 signal from audio
///
/// Implements WSJT-X subtraction algorithm:
/// 1. Cross-correlate audio with reference signal to find complex amplitude
/// 2. Low-pass filter the amplitude to get smooth envelope
/// 3. Reconstruct signal and subtract: dd(t) = dd(t) - 2*REAL{cref*cfilt}
///
/// # Arguments
///
/// * `audio` - Original audio buffer (will be modified in-place)
/// * `cref` - Complex reference signal from synthesize_ft8_signal
/// * `nwave` - Number of samples in cref
/// * `dt` - Time offset in seconds where signal starts
/// * `f0` - Center frequency in Hz
///
/// # Returns
///
/// True if subtraction successful, false if signal outside buffer bounds
pub fn subtract_ft8_signal(
    audio: &mut [f32],
    cref: &[(f32, f32)],
    nwave: usize,
    dt: f32,
    _f0: f32, // Kept for future refinement support
) -> Result<bool, String> {
    const NMAX: usize = 15 * 12000;
    const NFILT: usize = 4000; // Filter length for smoothing

    if audio.len() != NMAX {
        return Err(format!("Audio buffer must be {} samples", NMAX));
    }

    // Convert time offset to sample index
    let nstart = (dt * SAMPLE_RATE).round() as i32;

    // Compute complex amplitude: camp(t) = dd(t) * conj(cref(t))
    let mut camp = vec![(0.0f32, 0.0f32); nwave];

    for i in 0..nwave {
        let j = nstart + i as i32;
        if j >= 0 && (j as usize) < audio.len() {
            let signal = audio[j as usize];
            let (cref_r, cref_i) = cref[i];
            // Complex conjugate multiply: signal * conj(cref)
            camp[i] = (signal * cref_r, -signal * cref_i);
        }
    }

    // Low-pass filter camp to get smooth amplitude envelope
    // Use simple boxcar filter for now (TODO: implement proper FFT-based filter like WSJT-X)
    let mut cfilt = vec![(0.0f32, 0.0f32); nwave];
    let half_filt = NFILT / 2;

    for i in 0..nwave {
        let mut sum_r = 0.0;
        let mut sum_i = 0.0;
        let mut count = 0;

        for k in 0..NFILT {
            let idx = i as i32 + k as i32 - half_filt as i32;
            if idx >= 0 && (idx as usize) < nwave {
                sum_r += camp[idx as usize].0;
                sum_i += camp[idx as usize].1;
                count += 1;
            }
        }

        if count > 0 {
            cfilt[i] = (sum_r / count as f32, sum_i / count as f32);
        }
    }

    // Subtract: dd(t) = dd(t) - 2*REAL{cref*cfilt}
    let mut subtracted = false;
    for i in 0..nwave {
        let j = nstart + i as i32;
        if j >= 0 && (j as usize) < audio.len() {
            let (cref_r, cref_i) = cref[i];
            let (cfilt_r, cfilt_i) = cfilt[i];

            // Complex multiply: cref * cfilt
            let z_r = cref_r * cfilt_r - cref_i * cfilt_i;

            // Subtract 2*REAL{z}
            audio[j as usize] -= 2.0 * z_r;
            subtracted = true;
        }
    }

    Ok(subtracted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gfsk_pulse() {
        // Pulse should peak at t=0
        let p0 = gfsk_pulse(2.0, 0.0);
        let p1 = gfsk_pulse(2.0, 1.0);
        assert!(p0 > p1);
        assert!(p0 > 0.9); // Should be close to 1.0 at center
    }

    #[test]
    fn test_synthesize_signal() {
        // Create simple tone sequence
        let mut tones = [0u8; 79];
        tones[0] = 3; // First Costas tone

        let mut output = vec![(0.0, 0.0); 180000];
        let result = synthesize_ft8_signal(&tones, 1000.0, &mut output);

        assert!(result.is_ok());
        let nwave = result.unwrap();
        assert_eq!(nwave, 79 * NSPS); // 79 symbols

        // Check that signal was generated
        let power: f32 = output[0..nwave].iter()
            .map(|(r, i)| r*r + i*i)
            .sum();
        assert!(power > 0.0);
    }
}
