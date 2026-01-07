use criterion::{black_box, criterion_group, criterion_main, Criterion};

/// Scalar LLR scaling (current implementation) - truly scalar, no vectorization
fn scale_llr_scalar(llr: &[f32], scale: f32, output: &mut [f32]) {
    // Use volatile reads/writes to prevent compiler optimizations
    for i in 0..llr.len() {
        let val = unsafe { std::ptr::read_volatile(&llr[i]) };
        let result = val * scale;
        unsafe { std::ptr::write_volatile(&mut output[i], result) };
    }
}

/// Optimized scalar with iterator (what the compiler might do)
fn scale_llr_auto(llr: &[f32], scale: f32, output: &mut [f32]) {
    for (out, &val) in output.iter_mut().zip(llr.iter()) {
        *out = val * scale;
    }
}

/// SIMD LLR scaling using x86_64 AVX2 intrinsics (8 floats at once)
#[cfg(target_arch = "x86_64")]
fn scale_llr_simd(llr: &[f32], scale: f32, output: &mut [f32]) {
    use std::arch::x86_64::*;
    
    unsafe {
        let scale_vec = _mm256_set1_ps(scale); // Broadcast scale to all 8 lanes
        
        // Process 8 elements at a time
        let chunks = llr.len() / 8;
        for i in 0..chunks {
            let offset = i * 8;
            
            // Load 8 floats from llr
            let input_vec = _mm256_loadu_ps(llr.as_ptr().add(offset));
            
            // Multiply by scale (one instruction!)
            let result_vec = _mm256_mul_ps(input_vec, scale_vec);
            
            // Store 8 results
            _mm256_storeu_ps(output.as_mut_ptr().add(offset), result_vec);
        }
        
        // Handle remainder (174 % 8 = 6 elements)
        let remainder_start = chunks * 8;
        for i in remainder_start..llr.len() {
            output[i] = llr[i] * scale;
        }
    }
}

/// SIMD LLR scaling using ARM NEON intrinsics (4 floats at once)
#[cfg(target_arch = "aarch64")]
fn scale_llr_simd(llr: &[f32], scale: f32, output: &mut [f32]) {
    use std::arch::aarch64::*;
    
    unsafe {
        let scale_vec = vdupq_n_f32(scale); // Broadcast scale to all 4 lanes
        
        // Process 4 elements at a time
        let chunks = llr.len() / 4;
        for i in 0..chunks {
            let offset = i * 4;
            
            // Load 4 floats from llr
            let input_vec = vld1q_f32(llr.as_ptr().add(offset));
            
            // Multiply by scale (one instruction!)
            let result_vec = vmulq_f32(input_vec, scale_vec);
            
            // Store 4 results
            vst1q_f32(output.as_mut_ptr().add(offset), result_vec);
        }
        
        // Handle remainder (174 % 4 = 2 elements)
        let remainder_start = chunks * 4;
        for i in remainder_start..llr.len() {
            output[i] = llr[i] * scale;
        }
    }
}

/// Fallback for other architectures
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn scale_llr_simd(llr: &[f32], scale: f32, output: &mut [f32]) {
    scale_llr_scalar(llr, scale, output);
}

fn benchmark_llr_scaling(c: &mut Criterion) {
    let llr: Vec<f32> = (0..174).map(|i| (i as f32) * 0.1).collect();
    let scale = 1.5f32;
    
    // Verify correctness first
    let mut scalar_out = vec![0.0f32; 174];
    let mut auto_out = vec![0.0f32; 174];
    let mut simd_out = vec![0.0f32; 174];
    scale_llr_scalar(&llr, scale, &mut scalar_out);
    scale_llr_auto(&llr, scale, &mut auto_out);
    scale_llr_simd(&llr, scale, &mut simd_out);
    
    for i in 0..174 {
        assert!((scalar_out[i] - simd_out[i]).abs() < 0.0001, 
                "Mismatch at index {}: scalar={}, simd={}", i, scalar_out[i], simd_out[i]);
        assert!((auto_out[i] - simd_out[i]).abs() < 0.0001, 
                "Mismatch at index {}: auto={}, simd={}", i, auto_out[i], simd_out[i]);
    }
    println!("✓ SIMD correctness verified");
    
    // Benchmark scalar version (truly scalar)
    c.bench_function("llr_scale_scalar_100x", |b| {
        b.iter(|| {
            let mut output = vec![0.0f32; 174];
            for _ in 0..100 {
                scale_llr_scalar(black_box(&llr), black_box(scale), black_box(&mut output));
            }
            black_box(output)
        })
    });
    
    // Benchmark auto-vectorized version
    c.bench_function("llr_scale_auto_100x", |b| {
        b.iter(|| {
            let mut output = vec![0.0f32; 174];
            for _ in 0..100 {
                scale_llr_auto(black_box(&llr), black_box(scale), black_box(&mut output));
            }
            black_box(output)
        })
    });
    
    // Benchmark explicit SIMD version
    c.bench_function("llr_scale_simd_100x", |b| {
        b.iter(|| {
            let mut output = vec![0.0f32; 174];
            for _ in 0..100 {
                scale_llr_simd(black_box(&llr), black_box(scale), black_box(&mut output));
            }
            black_box(output)
        })
    });
}

criterion_group!(benches, benchmark_llr_scaling);
criterion_main!(benches);
