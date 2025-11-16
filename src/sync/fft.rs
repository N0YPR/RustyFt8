///! FFT utilities for FT8 signal processing
///!
///! Provides in-place FFT implementations for real and complex signals.

extern crate alloc;
use alloc::string::String;

/// In-place real-to-complex FFT (Cooley-Tukey algorithm)
///
/// # Arguments
/// * `real` - Real part (input/output)
/// * `imag` - Imaginary part (input/output, should be zeros on input)
/// * `n` - FFT size (must be power of 2)
pub(crate) fn fft_real(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    if n & (n - 1) != 0 {
        return Err(alloc::format!("FFT size must be power of 2, got {}", n));
    }

    // Bit-reversal permutation
    let mut j = 0;
    for i in 0..n - 1 {
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
        let mut k = n / 2;
        while k <= j {
            j -= k;
            k /= 2;
        }
        j += k;
    }

    // Cooley-Tukey decimation-in-time FFT
    let mut len = 2;
    while len <= n {
        let half_len = len / 2;
        let angle = -2.0 * core::f32::consts::PI / len as f32;

        for i in (0..n).step_by(len) {
            let mut k = 0;
            for j in i..i + half_len {
                let theta = angle * k as f32;
                let wr = libm::cosf(theta);
                let wi = libm::sinf(theta);

                let t_real = wr * real[j + half_len] - wi * imag[j + half_len];
                let t_imag = wr * imag[j + half_len] + wi * real[j + half_len];

                real[j + half_len] = real[j] - t_real;
                imag[j + half_len] = imag[j] - t_imag;
                real[j] += t_real;
                imag[j] += t_imag;

                k += 1;
            }
        }
        len *= 2;
    }

    Ok(())
}

/// Complex-to-complex FFT
pub(crate) fn fft_complex(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    fft_real(real, imag, n)
}

/// Complex-to-complex inverse FFT
pub(crate) fn fft_complex_inverse(real: &mut [f32], imag: &mut [f32], n: usize) -> Result<(), String> {
    // Conjugate input
    for i in 0..n {
        imag[i] = -imag[i];
    }

    // Forward FFT
    fft_real(real, imag, n)?;

    // Conjugate output and scale
    for i in 0..n {
        imag[i] = -imag[i] / n as f32;
        real[i] /= n as f32;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;

    #[test]
    fn test_fft_real_dc() {
        let mut real = alloc::vec![1.0f32; 8];
        let mut imag = alloc::vec![0.0f32; 8];

        fft_real(&mut real, &mut imag, 8).unwrap();

        // DC component should be 8.0
        assert!((real[0] - 8.0).abs() < 0.001);
        // Other bins should be near zero
        for i in 1..8 {
            assert!(real[i].abs() < 0.001);
            assert!(imag[i].abs() < 0.001);
        }
    }

    #[test]
    fn test_fft_real_sine() {
        let n = 64;
        let mut real = alloc::vec![0.0f32; n];
        let mut imag = alloc::vec![0.0f32; n];

        // Generate sine wave at bin 5
        let freq = 5.0;
        for i in 0..n {
            let phase = 2.0 * core::f32::consts::PI * freq * i as f32 / n as f32;
            real[i] = libm::sinf(phase);
        }

        fft_real(&mut real, &mut imag, n).unwrap();

        // Peak should be at bin 5
        let mut max_mag = 0.0f32;
        let mut max_bin = 0;
        for i in 0..n {
            let mag = libm::sqrtf(real[i] * real[i] + imag[i] * imag[i]);
            if mag > max_mag {
                max_mag = mag;
                max_bin = i;
            }
        }

        assert_eq!(max_bin, 5, "Peak should be at bin 5");
    }
}
