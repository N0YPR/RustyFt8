use rustyft8::{pulse, subtract};

#[test]
fn debug_subtract_simple() {
    // Create a very simple test: single frequency sine wave
    // Generate reference signal at 1500 Hz
    let tones = [3u8; 79];

    let mut pulse_buf = vec![0.0f32; 3 * 1920];
    pulse::compute_pulse(&mut pulse_buf, 2.0, 1920).unwrap();

    // Generate as real signal
    let mut real_signal = vec![0.0f32; 79 * 1920];
    pulse::generate_waveform(
        &tones,
        &mut real_signal,
        &pulse_buf,
        1500.0,
        12000.0,
        1920,
    ).unwrap();

    // Also generate as complex for comparison
    let mut complex_signal = vec![(0.0f32, 0.0f32); 79 * 1920];
    pulse::generate_complex_waveform(
        &tones,
        &mut complex_signal,
        &pulse_buf,
        1500.0,
        12000.0,
        1920,
    ).unwrap();

    println!("Real signal samples:");
    for i in (10000..10010).step_by(2) {
        println!("  [{}] = {:.6}", i, real_signal[i]);
    }

    println!("\nComplex signal samples:");
    for i in (10000..10010).step_by(2) {
        let mag = (complex_signal[i].0.powi(2) + complex_signal[i].1.powi(2)).sqrt();
        println!("  [{}] = ({:.6}, {:.6}) mag={:.6}", i, complex_signal[i].0, complex_signal[i].1, mag);
    }

    // Create audio buffer
    let mut audio = vec![0.0f32; 15 * 12000];
    let start = 6000; // 0.5 seconds

    // Place signal
    for (i, &sample) in real_signal.iter().enumerate() {
        audio[start + i] = sample;
    }

    let power_before: f32 = audio[start..start + real_signal.len()]
        .iter()
        .map(|&x| x * x)
        .sum();

    println!("\nBefore subtraction:");
    println!("  Power: {:.2e}", power_before);
    println!("  Sample at 10000: {:.6}", audio[start + 10000]);
    println!("  Sample at 10001: {:.6}", audio[start + 10001]);

    // Perform subtraction
    let result = subtract::subtract_ft8_signal(&mut audio, &tones, 1500.0, 0.5);
    assert!(result.is_ok(), "Subtraction failed: {:?}", result);

    let power_after: f32 = audio[start..start + real_signal.len()]
        .iter()
        .map(|&x| x * x)
        .sum();

    println!("\nAfter subtraction:");
    println!("  Power: {:.2e}", power_after);
    println!("  Sample at 10000: {:.6}", audio[start + 10000]);
    println!("  Sample at 10001: {:.6}", audio[start + 10001]);

    let reduction_db = 10.0 * (power_after / power_before).log10();
    println!("\nPower change: {:.1} dB", reduction_db);

    // Check power outside signal region
    let power_before_signal: f32 = audio[0..start].iter().map(|&x| x * x).sum();
    let power_after_signal: f32 = audio[start + real_signal.len()..].iter().map(|&x| x * x).sum();
    println!("\nPower outside signal region:");
    println!("  Before signal (0..{}): {:.2e}", start, power_before_signal);
    println!("  After signal ({}..): {:.2e}", start + real_signal.len(), power_after_signal);

    if reduction_db > 0.0 {
        println!("\nERROR: Signal got LOUDER instead of quieter!");
        println!("This suggests a phase error in reconstruction");
    } else if reduction_db < -20.0 {
        println!("\nSUCCESS: Signal reduced by more than 20 dB");
    } else {
        println!("\nPARTIAL: Signal reduced but less than 20 dB");
    }
}
