///! FFT utilities for FT8 signal processing
///!
///! Provides FFT implementations using RustFFT for high performance.

use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;

/// Cache of forward FFT plans
static FFT_FORWARD_CACHE: Lazy<Mutex<HashMap<usize, Arc<dyn Fft<f32>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Cache of inverse FFT plans
static FFT_INVERSE_CACHE: Lazy<Mutex<HashMap<usize, Arc<dyn Fft<f32>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Get or create a forward FFT plan for the given size
fn get_forward_plan(n: usize) -> Arc<dyn Fft<f32>> {
    let mut cache = FFT_FORWARD_CACHE.lock().unwrap();

    if let Some(plan) = cache.get(&n) {
        return Arc::clone(plan);
    }

    let mut planner = FftPlanner::new();
    let plan = planner.plan_fft_forward(n);
    cache.insert(n, Arc::clone(&plan));
    plan
}

/// Get or create an inverse FFT plan for the given size
fn get_inverse_plan(n: usize) -> Arc<dyn Fft<f32>> {
    let mut cache = FFT_INVERSE_CACHE.lock().unwrap();

    if let Some(plan) = cache.get(&n) {
        return Arc::clone(plan);
    }

    let mut planner = FftPlanner::new();
    let plan = planner.plan_fft_inverse(n);
    cache.insert(n, Arc::clone(&plan));
    plan
}

/// In-place real-to-complex FFT using RustFFT
///
/// # Arguments
/// * `real` - Real part (input/output)
/// * `imag` - Imaginary part (input/output, should be zeros on input)
/// * `n` - FFT size (must be power of 2)
pub(crate) fn fft_real(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    if n & (n - 1) != 0 {
        return Err(format!("FFT size must be power of 2, got {}", n));
    }

    if real.len() < n || imag.len() < n {
        return Err(format!(
            "Buffers too small: real={}, imag={}, need={}",
            real.len(),
            imag.len(),
            n
        ));
    }

    // Convert separate real/imag arrays to Complex
    let mut buffer: Vec<Complex<f32>> = (0..n)
        .map(|i| Complex::new(real[i], imag[i]))
        .collect();

    // Perform FFT using cached plan
    let fft = get_forward_plan(n);
    fft.process(&mut buffer);

    // Convert back to separate arrays
    for i in 0..n {
        real[i] = buffer[i].re;
        imag[i] = buffer[i].im;
    }

    Ok(())
}

/// Complex-to-complex FFT
///
/// Same as fft_real but handles complex input
pub(crate) fn fft_complex(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    fft_real(real, imag, n)
}

/// Complex-to-complex inverse FFT
pub(crate) fn fft_complex_inverse(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    if n & (n - 1) != 0 {
        return Err(format!("FFT size must be power of 2, got {}", n));
    }

    if real.len() < n || imag.len() < n {
        return Err(format!(
            "Buffers too small: real={}, imag={}, need={}",
            real.len(),
            imag.len(),
            n
        ));
    }

    // Convert separate real/imag arrays to Complex
    let mut buffer: Vec<Complex<f32>> = (0..n)
        .map(|i| Complex::new(real[i], imag[i]))
        .collect();

    // Perform inverse FFT using cached plan
    let fft = get_inverse_plan(n);
    fft.process(&mut buffer);

    // Scale and convert back to separate arrays
    let scale = 1.0 / n as f32;
    for i in 0..n {
        real[i] = buffer[i].re * scale;
        imag[i] = buffer[i].im * scale;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_real_dc() {
        let mut real = vec![1.0f32; 32];
        let mut imag = vec![0.0f32; 32];

        fft_real(&mut real, &mut imag, 32).unwrap();

        // DC component should be 32.0 (sum of inputs)
        assert!((real[0] - 32.0).abs() < 0.1, "DC component: {}", real[0]);
    }

    #[test]
    fn test_fft_real_sine() {
        let n = 32;
        let mut real = vec![0.0f32; n];
        let mut imag = vec![0.0f32; n];

        // Generate sine wave at bin 5
        let freq = 5.0;
        for i in 0..n {
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / n as f32;
            real[i] = f32::sin(phase);
        }

        fft_real(&mut real, &mut imag, n).unwrap();

        // Find peak
        let mut max_mag = 0.0f32;
        let mut max_bin = 0;
        for i in 0..n {
            let mag = (real[i] * real[i] + imag[i] * imag[i]).sqrt();
            if mag > max_mag {
                max_mag = mag;
                max_bin = i;
            }
        }

        // Peak should be at bin 5
        assert!(
            (max_bin as i32 - 5).abs() <= 1,
            "Peak at bin {}, expected near 5",
            max_bin
        );
    }

    #[test]
    fn test_ifft_roundtrip() {
        let n = 32;
        let mut real = vec![0.0f32; n];
        let mut imag = vec![0.0f32; n];

        // Create a simple signal
        for i in 0..n {
            real[i] = (i as f32).sin();
        }

        let original_real = real.clone();

        // Forward FFT
        fft_complex(&mut real, &mut imag, n).unwrap();

        // Inverse FFT
        fft_complex_inverse(&mut real, &mut imag, n).unwrap();

        // Should recover original (with small numerical error)
        for i in 0..n {
            assert!(
                (real[i] - original_real[i]).abs() < 0.01,
                "Roundtrip failed at {}: {} vs {}",
                i,
                real[i],
                original_real[i]
            );
        }
    }

    #[test]
    fn test_large_fft_262144() {
        // Test the critical 262,144-point FFT size
        let n = 262144;
        let mut real = vec![0.0f32; n];
        let mut imag = vec![0.0f32; n];

        // Simple DC signal
        for i in 0..n {
            real[i] = 1.0;
        }

        fft_real(&mut real, &mut imag, n).unwrap();

        // DC component should be n
        assert!(
            (real[0] - n as f32).abs() < 1.0,
            "DC component: {}, expected {}",
            real[0],
            n
        );
    }
}
