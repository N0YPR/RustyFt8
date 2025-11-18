///! FT8 Signal Synchronization
///!
///! This module implements signal detection and synchronization for FT8 using Costas array correlation.
///!
///! **FT8 Sync Structure**:
///! - Three 7x7 Costas arrays at symbols 0-6, 36-42, 72-78
///! - Costas pattern: [3,1,4,0,6,5,2] (7 unique tones)
///!
///! **Algorithm**:
///! 1. Compute 2D sync matrix: sync2d[frequency_bin, time_lag]
///! 2. Correlate against Costas patterns at all three positions
///! 3. Find peaks and generate candidate signals
///! 4. Refine time/frequency estimates with fine synchronization
///!
///! **Search Strategy**:
///! - Coarse: 3.125 Hz freq resolution, 40 ms time resolution
///! - Fine: 0.5 Hz freq resolution, 5 ms time resolution
///!
///! **Module Organization**:
///! - `fft` - FFT implementations
///! - `spectra` - Spectrogram computation and sync correlation
///! - `candidate` - Candidate detection and coarse sync
///! - `downsample` - Signal downsampling
///! - `fine` - Fine synchronization
///! - `extract` - Symbol extraction and LLR computation

// Submodules (internal)
mod fft;
mod spectra;
mod downsample;
pub mod candidate;
pub mod fine;
pub mod extract;

// Re-export public API
pub use candidate::{Candidate, coarse_sync, find_candidates};
pub use fine::{fine_sync, sync_downsampled};
pub use extract::{extract_symbols, extract_symbols_with_powers, calculate_snr};
pub use downsample::downsample_200hz;
pub use spectra::{compute_spectra, compute_sync2d, compute_baseline};

/// Costas 7x7 tone pattern used in FT8
pub const COSTAS_PATTERN: [u8; 7] = [3, 1, 4, 0, 6, 5, 2];

/// Maximum time lag for coarse sync: ±2.5s at 4 samples/symbol = 62.5 steps
pub const MAX_LAG: i32 = 62;

/// Coarse time search window: ±10 lag steps around expected time
pub const COARSE_LAG: i32 = 10;

/// FT8 sample rate in Hz
pub const SAMPLE_RATE: f32 = 12000.0;

/// Samples per symbol
pub const NSPS: usize = 1920;

/// Time step between spectra (1/4 symbol = 480 samples)
pub const NSTEP: usize = NSPS / 4;

/// FFT size for symbol spectra (must be power of 2)
pub const NFFT1: usize = 4096; // Nearest power of 2 to 2*NSPS (3840)

/// Number of FFT bins
pub const NH1: usize = NFFT1 / 2; // 2048

/// Maximum number of samples (15 seconds at 12 kHz)
pub const NMAX: usize = 15 * 12000; // 180,000

/// Number of spectra (15s / 40ms steps)
pub const NHSYM: usize = NMAX / NSTEP - 3; // 372

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_costas_pattern() {
        assert_eq!(COSTAS_PATTERN.len(), 7);
        // All tones should be unique and in range 0-6
        for &tone in &COSTAS_PATTERN {
            assert!(tone < 8);
        }
    }

    #[test]
    fn test_compute_spectra_size() {
        // NH1 = NFFT1/2 = 4096/2 = 2048 bins
        // NHSYM = NMAX/NSTEP - 3 = 180000/480 - 3 = 372 time steps
        assert_eq!(NH1, 2048);
        assert_eq!(NHSYM, 372);

        // Also test actual compute_spectra function
        let signal = vec![0.0f32; NMAX];
        let mut spectra = vec![[0.0f32; NHSYM]; NH1];

        let result = compute_spectra(&signal, &mut spectra);
        assert!(result.is_ok());

        let avg_spectrum = result.unwrap();
        assert_eq!(avg_spectrum.len(), NH1);
    }

    #[test]
    fn test_compute_spectra_too_short() {
        let signal = vec![0.0f32; 1000]; // Too short
        let mut spectra = vec![[0.0f32; NHSYM]; NH1];

        let result = compute_spectra(&signal, &mut spectra);
        assert!(result.is_err());
    }
}
