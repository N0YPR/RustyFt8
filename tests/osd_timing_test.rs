use rustyft8::{ldpc, message, crc};
use bitvec::prelude::*;
use std::time::Instant;

/// Helper function to create a 91-bit message from text
fn create_message91(text: &str) -> Result<BitVec<u8, Msb0>, String> {
    let mut msg77 = bitvec![u8, Msb0; 0; 77];
    let mut cache = message::CallsignHashCache::new();
    message::encode(text, &mut msg77, &mut cache)?;

    let crc_value = crc::crc14(&msg77);
    let mut message91 = bitvec![u8, Msb0; 0; 91];
    message91[0..77].copy_from_bitslice(&msg77);

    for i in 0..14 {
        message91.set(77 + i, ((crc_value >> (13 - i)) & 1) != 0);
    }

    Ok(message91)
}

#[test]
fn test_osd_timing() {
    let text = "CQ N0YPR DM42";
    let message91 = create_message91(text).expect("Failed to encode message");

    let mut codeword = bitvec![u8, Msb0; 0; 174];
    ldpc::encode(&message91, &mut codeword);

    // Create slightly noisy LLRs
    let mut llr = vec![0.0f32; 174];
    for i in 0..174 {
        llr[i] = if codeword[i] { 3.0 } else { -3.0 };
    }

    println!("\n=== OSD Timing Test ===\n");

    // Time order-0
    let start = Instant::now();
    let result = ldpc::osd_decode(&llr, 0);
    let duration = start.elapsed();

    println!("Order-0 decode: {:?}", duration);
    if result.is_some() {
        println!("Result: SUCCESS");
    } else {
        println!("Result: FAILED");
    }

    // Run 10 times to get average
    println!("\nRunning 10 iterations for average timing:");
    let start = Instant::now();
    for i in 0..10 {
        let _result = ldpc::osd_decode(&llr, 0);
    }
    let total = start.elapsed();
    println!("Total: {:?}, Average: {:?}", total, total / 10);

    // Test order-1
    println!("\nTesting Order-1:");
    let start = Instant::now();
    let result = ldpc::osd_decode(&llr, 1);
    let duration = start.elapsed();
    println!("Order-1 decode: {:?}", duration);
    if result.is_some() {
        println!("Result: SUCCESS");
    } else {
        println!("Result: FAILED");
    }
}
