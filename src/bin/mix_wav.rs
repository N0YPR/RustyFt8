///! Mix two WAV files by adding their samples

use std::env;
use hound::{WavReader, WavWriter, SampleFormat, WavSpec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <input1.wav> <input2.wav> <output.wav>", args[0]);
        std::process::exit(1);
    }

    let input1 = &args[1];
    let input2 = &args[2];
    let output = &args[3];

    println!("Mixing {} + {} → {}", input1, input2, output);

    // Read first file
    let mut reader1 = WavReader::open(input1)?;
    let spec1 = reader1.spec();
    let samples1: Vec<i16> = reader1.samples::<i16>().collect::<Result<Vec<_>, _>>()?;

    // Read second file
    let mut reader2 = WavReader::open(input2)?;
    let spec2 = reader2.spec();
    let samples2: Vec<i16> = reader2.samples::<i16>().collect::<Result<Vec<_>, _>>()?;

    // Verify specs match
    if spec1.sample_rate != spec2.sample_rate {
        eprintln!("Error: Sample rates don't match: {} vs {}", spec1.sample_rate, spec2.sample_rate);
        std::process::exit(1);
    }

    println!("  File 1: {} samples", samples1.len());
    println!("  File 2: {} samples", samples2.len());

    // Pad to same length
    let max_len = samples1.len().max(samples2.len());
    let mut mixed = Vec::with_capacity(max_len);

    for i in 0..max_len {
        let s1 = samples1.get(i).copied().unwrap_or(0) as i32;
        let s2 = samples2.get(i).copied().unwrap_or(0) as i32;
        let sum = s1 + s2;
        // Clip to i16 range
        let clipped = sum.clamp(-32768, 32767) as i16;
        mixed.push(clipped);
    }

    // Write output
    let spec = WavSpec {
        channels: 1,
        sample_rate: spec1.sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(output, spec)?;
    for sample in mixed {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;

    println!("  Output: {} samples at {} Hz", max_len, spec1.sample_rate);
    println!("Done!");

    Ok(())
}
