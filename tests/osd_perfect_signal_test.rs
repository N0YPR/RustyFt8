use rustyft8::{ldpc, message, crc};
use bitvec::prelude::*;

/// Helper function to create a 91-bit message from text
fn create_message91(text: &str) -> Result<BitVec<u8, Msb0>, String> {
    // Encode to 77 bits
    let mut msg77 = bitvec![u8, Msb0; 0; 77];
    let mut cache = message::CallsignHashCache::new();
    message::encode(text, &mut msg77, &mut cache)?;

    // Add CRC to get 91 bits
    let crc_value = crc::crc14(&msg77);

    // Create 91-bit message
    let mut message91 = bitvec![u8, Msb0; 0; 91];
    message91[0..77].copy_from_bitslice(&msg77);

    // Add CRC in bits 77-90
    for i in 0..14 {
        message91.set(77 + i, ((crc_value >> (13 - i)) & 1) != 0);
    }

    Ok(message91)
}

#[test]
fn test_osd_on_perfect_signal() {
    let text = "CQ N0YPR DM42";
    println!("Testing OSD on perfect signal: \"{}\"", text);

    // Create 91-bit message with CRC
    let message91 = create_message91(text).expect("Failed to encode message");

    // Encode with LDPC to get 174-bit codeword
    let mut codeword = bitvec![u8, Msb0; 0; 174];
    ldpc::encode(&message91, &mut codeword);

    // Create perfect LLRs (positive for 1, negative for 0)
    let mut llr = vec![0.0f32; 174];
    for i in 0..174 {
        llr[i] = if codeword[i] { 10.0 } else { -10.0 };
    }

    // Try decoding with BP first
    println!("\n1. Trying BP decode...");
    let bp_result = ldpc::decode(&llr, 200);
    if let Some((decoded, iters)) = &bp_result {
        println!("   ✓ BP decoded successfully in {} iterations", iters);
        assert_eq!(*decoded, message91, "BP should decode correctly");
    } else {
        println!("   ✗ BP failed (unexpected for perfect signal)");
    }

    // Try OSD order-0
    println!("\n2. Trying OSD order-0...");
    let osd_result = ldpc::osd_decode(&llr, 0);
    if let Some(decoded) = osd_result {
        println!("   ✓ OSD decoded successfully");
        assert_eq!(decoded, message91, "OSD should decode correctly");
    } else {
        println!("   ✗ OSD failed");
        panic!("OSD should be able to decode a perfect signal!");
    }
}

#[test]
fn test_osd_on_noisy_signal() {
    let text = "CQ N0YPR DM42";
    println!("\nTesting OSD on noisy signal with 5 bit errors: \"{}\"", text);

    // Create 91-bit message with CRC
    let message91 = create_message91(text).expect("Failed to encode message");

    // Encode with LDPC to get 174-bit codeword
    let mut codeword = bitvec![u8, Msb0; 0; 174];
    ldpc::encode(&message91, &mut codeword);

    // Create LLRs with 5 errors (flip 5 bits)
    let mut llr = vec![0.0f32; 174];
    for i in 0..174 {
        llr[i] = if codeword[i] { -5.0 } else { 5.0 };
    }

    // Introduce 5 bit errors at positions 10, 30, 50, 70, 90
    for &pos in &[10, 30, 50, 70, 90] {
        llr[pos] = -llr[pos] * 0.8; // Flip with reduced confidence
    }

    // Try BP first
    println!("\n1. Trying BP decode...");
    let bp_result = ldpc::decode(&llr, 200);
    if bp_result.is_some() {
        println!("   ✓ BP decoded successfully");
    } else {
        println!("   ✗ BP failed");

        // Try OSD
        println!("\n2. Trying OSD order-1...");
        let osd_result = ldpc::osd_decode(&llr, 1);
        if osd_result.is_some() {
            println!("   ✓ OSD recovered the message!");
        } else {
            println!("   ✗ OSD also failed");
        }
    }
}
