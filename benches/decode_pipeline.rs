//! Comprehensive benchmarks for FT8 decoding pipeline
//!
//! Measures performance of individual components to identify bottlenecks:
//! - Full end-to-end decode
//! - Coarse sync
//! - FFT operations
//! - Spectra computation

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use rustyft8::{decode_ft8, DecoderConfig};
use rustyft8::sync::{coarse_sync, compute_spectra, NHSYM};
use rustfft::{FftPlanner, num_complex::Complex};
use std::fs::File;

/// Load real FT8 recording for benchmarking
fn load_test_recording() -> Vec<f32> {
    // Load the 15-second recording with 22 messages
    let path = "tests/test_data/210703_133430.wav";
    let file = File::open(path).expect("Failed to open test recording");
    let reader = hound::WavReader::new(file).expect("Failed to read WAV");
    
    reader
        .into_samples::<i16>()
        .map(|s| s.expect("Failed to read sample") as f32 / 32768.0)
        .collect()
}

/// Benchmark full end-to-end decode
fn bench_full_decode(c: &mut Criterion) {
    let signal = load_test_recording();
    
    let mut group = c.benchmark_group("full_decode");
    group.sample_size(10); // Reduce sample size for slower benchmarks
    
    // Default configuration (decodes ~22 messages)
    group.bench_function("default_config", |b| {
        b.iter(|| {
            let config = DecoderConfig::default();
            decode_ft8(black_box(&signal), black_box(&config), |_| true)
        });
    });
    
    // Decode fewer candidates (faster)
    group.bench_function("top_10_candidates", |b| {
        b.iter(|| {
            let config = DecoderConfig {
                decode_top_n: 10,
                ..DecoderConfig::default()
            };
            decode_ft8(black_box(&signal), black_box(&config), |_| true)
        });
    });
    
    // Decode with narrow frequency range
    group.bench_function("narrow_freq_range", |b| {
        b.iter(|| {
            let config = DecoderConfig {
                freq_min: 1000.0,
                freq_max: 2000.0,
                ..DecoderConfig::default()
            };
            decode_ft8(black_box(&signal), black_box(&config), |_| true)
        });
    });
    
    group.finish();
}

/// Benchmark coarse sync stage
fn bench_coarse_sync(c: &mut Criterion) {
    let signal = load_test_recording();
    
    let mut group = c.benchmark_group("coarse_sync");
    
    group.bench_function("full_bandwidth", |b| {
        b.iter(|| {
            coarse_sync(
                black_box(&signal),
                black_box(200.0),
                black_box(3500.0),
                black_box(4.0),
                black_box(50)
            )
        });
    });
    
    group.bench_function("narrow_bandwidth", |b| {
        b.iter(|| {
            coarse_sync(
                black_box(&signal),
                black_box(1000.0),
                black_box(2000.0),
                black_box(4.0),
                black_box(50)
            )
        });
    });
    
    group.finish();
}

/// Benchmark FFT operations (critical for sync stage)
fn bench_fft_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("fft");
    
    // Test various FFT sizes used in FT8
    for size in [512, 1024, 2048, 4096, 8192] {
        group.bench_with_input(
            BenchmarkId::new("forward", size),
            &size,
            |b, &n| {
                let mut planner = FftPlanner::new();
                let fft = planner.plan_fft_forward(n);
                let mut buffer = vec![Complex::new(1.0, 0.0); n];
                
                b.iter(|| {
                    fft.process(black_box(&mut buffer));
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark spectra computation (preprocessing stage)
fn bench_spectra(c: &mut Criterion) {
    let signal = load_test_recording();
    
    let mut group = c.benchmark_group("spectra");
    
    group.bench_function("full_signal", |b| {
        b.iter(|| {
            let mut spectra = [[0.0f32; NHSYM]; 8];
            compute_spectra(
                black_box(&signal),
                black_box(&mut spectra)
            )
        });
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_full_decode,
    bench_coarse_sync,
    bench_fft_operations,
    bench_spectra
);
criterion_main!(benches);
