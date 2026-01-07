use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustyft8::{decode_ft8, DecoderConfig, DecodedMessage};
use std::time::{Duration, Instant};

// Simple test data reading
fn read_wav_file(path: &str) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Failed to open WAV: {}", e))?;
    
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();
    
    Ok(samples)
}

fn normalize_signal_length(signal: Vec<f32>) -> Vec<f32> {
    const TARGET_SAMPLES: usize = 180000; // 15 seconds at 12kHz
    
    if signal.len() >= TARGET_SAMPLES {
        signal[..TARGET_SAMPLES].to_vec()
    } else {
        let mut normalized = signal.clone();
        normalized.resize(TARGET_SAMPLES, 0.0);
        normalized
    }
}

fn profile_ft8_decode(c: &mut Criterion) {
    // Load test file
    let wav_path = "tests/test_data/210703_133430.wav";
    let signal = read_wav_file(wav_path)
        .expect("Failed to read WAV");
    let signal_15s = normalize_signal_length(signal);
    
    let config = DecoderConfig::default();
    
    // Warmup run
    let mut warmup_count = 0;
    let _ = decode_ft8(&signal_15s, &config, |_msg| {
        warmup_count += 1;
        true
    });
    println!("Warmup decoded {} messages", warmup_count);
    
    // Detailed profiling run with phase timing
    println!("\n=== Detailed Profiling ===");
    let start = Instant::now();
    
    let mut decode_count = 0;
    let _ = decode_ft8(&signal_15s, &config, |msg| {
        decode_count += 1;
        true
    });
    
    let total_time = start.elapsed();
    println!("Total time: {:?}", total_time);
    println!("Messages: {}", decode_count);
    println!("Avg per message: {:?}", total_time / decode_count.max(1) as u32);
    
    // Benchmark
    c.bench_function("decode_210703_recording", |b| {
        b.iter(|| {
            let mut count = 0;
            decode_ft8(black_box(&signal_15s), black_box(&config), |_msg| {
                count += 1;
                true
            })
        });
    });
}

criterion_group!{
    name = benches;
    config = Criterion::default().sample_size(10).warm_up_time(Duration::from_secs(1));
    targets = profile_ft8_decode
}
criterion_main!(benches);
