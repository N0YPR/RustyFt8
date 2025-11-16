///! FFT utilities for FT8 signal processing
///!
///! Provides custom FFT implementations optimized for no_std environments.


/// In-place real-to-complex FFT using Cooley-Tukey radix-2 algorithm
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
                let wr = f32::cos(theta);
                let wi = f32::sin(theta);
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

    // Conjugate input
    for i in 0..n {
        imag[i] = -imag[i];
    }

    // Forward FFT
    fft_real(real, imag, n)?;

    // Conjugate and scale output
    let scale = 1.0 / n as f32;
    for i in 0..n {
        real[i] *= scale;
        imag[i] *= -scale;
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
            let phase = 2.0 * core::f32::consts::PI * freq * i as f32 / n as f32;
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
}
