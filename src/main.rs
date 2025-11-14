use rustyft8::message::{encode, decode, CallsignHashCache};
use bitvec::prelude::*;

fn main() {
    // Example usage
    let test_messages = vec![
        "CQ N0YPR DM42",
        "CQ SOTA N0YPR DM42",
        "CQ W1ABC FN31",
    ];
    
    println!("RustyFt8 - FT8 Message Parser and Encoder\n");
    
    let mut cache = CallsignHashCache::new();
    
    for msg_text in test_messages {
        // Create storage for the 77-bit message
        let mut storage = bitarr![u8, Msb0; 0; 80];  // 10 bytes
        
        match encode(msg_text, &mut storage[0..77], Some(&mut cache)) {
            Ok(()) => {
                let bits = &storage[0..77];
                
                // Convert to binary string
                let binary: String = (0..77)
                    .map(|i| if bits[i] { '1' } else { '0' })
                    .collect();
                
                println!("Input:   {}", msg_text);
                
                // Decode the message back to text
                match decode(bits, Some(&cache)) {
                    Ok(decoded_text) => println!("Decoded: {}", decoded_text),
                    Err(e) => println!("Decoded: Error - {}", e),
                }
                
                println!("Bits:    {}", binary);
            }
            Err(e) => println!("Error: {}", e),
        }
        println!();
    }
}
