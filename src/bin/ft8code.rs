//! FT8 Message Encoder - Shows encoding details like WSJT-X ft8code
//!
//! Encodes FT8 messages and displays the complete bit breakdown:
//! - Source-encoded 77-bit message
//! - 14-bit CRC
//! - 83 LDPC parity bits
//! - 79 channel symbols (tones 0-7)
//! - Message type (i3.n3)
//!
//! Usage:
//!   ft8code "message text"
//!   ft8code -T              # Show all message type examples
//!
//! Examples:
//!   ft8code "CQ N0YPR DM42"
//!   ft8code "N0YPR K1ABC RR73"
//!   ft8code "CQ SOTA N0YPR/R DM42"

use rustyft8::{crc, ldpc, message, symbol};
use bitvec::prelude::*;
use std::env;

/// Message type descriptions
fn get_message_type(msg77: &BitSlice<u8, Msb0>) -> (String, String) {
    // Extract i3 (last 3 bits)
    let i3 = (msg77[74] as u8) << 2 | (msg77[75] as u8) << 1 | (msg77[76] as u8);

    // For i3=0, extract n3 (bits 71-73)
    let n3 = if i3 == 0 {
        Some((msg77[71] as u8) << 2 | (msg77[72] as u8) << 1 | (msg77[73] as u8))
    } else {
        None
    };

    let type_code = if let Some(n3_val) = n3 {
        format!("{}.{}", i3, n3_val)
    } else {
        format!("{}. ", i3)
    };

    let type_name = match (i3, n3) {
        (0, Some(0)) => "Free text",
        (0, Some(1)) => "DXpedition mode",
        (0, Some(2)) => "EU VHF Contest",
        (0, Some(3)) => "ARRL Field Day",
        (0, Some(4)) => "ARRL Field Day",
        (0, Some(5)) => "Telemetry",
        (0, Some(_)) => "Undefined type",
        (1, _) => "Standard msg",
        (2, _) => "EU VHF Contest",
        (3, _) => "ARRL RTTY Roundup",
        (4, _) => "Nonstandard call",
        (5, _) => "EU VHF Contest",
        _ => "Undefined type",
    };

    (type_code, type_name.to_string())
}

/// Format bits as binary string
fn bits_to_string(bits: &BitSlice<u8, Msb0>) -> String {
    bits.iter().map(|b| if *b { '1' } else { '0' }).collect()
}

/// Encode and display message details
fn encode_message(msg_text: &str) -> Result<(), String> {
    // Step 1: Encode text to 77-bit message
    let mut msg77_storage = bitarr![u8, Msb0; 0; 80];
    let msg77 = &mut msg77_storage[..77];
    message::encode(msg_text, msg77)?;

    // Step 2: Calculate CRC-14
    let crc_value = crc::crc14(msg77);

    // Step 3: Combine message + CRC into 91 bits
    let mut msg91_storage = bitarr![u8, Msb0; 0; 96];
    let msg91 = &mut msg91_storage[..91];
    msg91[..77].copy_from_bitslice(msg77);
    for i in 0..14 {
        msg91.set(77 + i, ((crc_value >> (13 - i)) & 1) != 0);
    }

    // Step 4: LDPC encode to 174 bits
    let mut codeword_storage = bitarr![u8, Msb0; 0; 176];
    let codeword = &mut codeword_storage[..174];
    ldpc::encode(msg91, codeword);

    // Step 5: Map to 79 symbols
    let mut symbols = [0u8; 79];
    symbol::map(codeword, &mut symbols)?;

    // Step 6: Decode back to verify
    let decoded_text = message::decode(msg77)?;

    // Get message type
    let (type_code, type_name) = get_message_type(msg77);

    // Display results
    println!("Input message:  {}", msg_text);
    println!("Decoded back:   {}", decoded_text);

    if msg_text != decoded_text {
        println!("Warning: Decoded message differs from input!");
    }

    println!();
    println!("Message type:   {} ({})", type_code, type_name);
    println!();

    // Source-encoded message (77 bits)
    println!("Source-encoded message, 77 bits:");
    let msg77_str = bits_to_string(msg77);
    println!("{}", msg77_str);
    println!();

    // 14-bit CRC
    println!("14-bit CRC:");
    let crc_bits = &codeword[77..91];
    let crc_str = bits_to_string(crc_bits);
    println!("{}", crc_str);
    println!();

    // 83 parity bits
    println!("83 Parity bits:");
    let parity_bits = &codeword[91..174];
    let parity_str = bits_to_string(parity_bits);
    println!("{}", parity_str);
    println!();

    // Channel symbols
    println!("Channel symbols (79 tones):");

    // Group symbols like WSJT-X: sync1 data1 sync2 data2 sync3
    // Sync arrays at: 0-6, 36-42, 72-78
    // Data blocks: 7-35, 43-71

    // First sync (0-6)
    for i in 0..7 {
        print!("{}", symbols[i]);
    }
    print!(" ");

    // First data block (7-35)
    for i in 7..36 {
        print!("{}", symbols[i]);
    }
    print!(" ");

    // Second sync (36-42) - note: only first symbol shown separately in WSJT-X
    for i in 36..43 {
        print!("{}", symbols[i]);
    }
    print!(" ");

    // Second data block (43-71)
    for i in 43..72 {
        print!("{}", symbols[i]);
    }
    print!(" ");

    // Third sync (72-78)
    for i in 72..79 {
        print!("{}", symbols[i]);
    }
    println!();

    Ok(())
}

/// Test messages covering all message types
fn get_test_messages() -> Vec<(&'static str, &'static str)> {
    vec![
        // Type 1: Standard messages
        ("CQ N0YPR DM42", "Standard CQ with grid"),
        ("N0YPR K1ABC RR73", "Standard QSO exchange"),
        ("K1ABC N0YPR -15", "Signal report"),
        ("N0YPR K1ABC/R FN42", "Rover suffix"),

        // Type 2: EU VHF Contest
        ("CQ SOTA N0YPR/R DM42", "SOTA with rover"),

        // Type 4: Nonstandard callsigns
        ("CQ KH1/KH7Z", "Nonstandard with prefix"),

        // Type 0.0: Free text
        ("TNX QSO 73", "Free text message"),

        // Special
        ("73", "Sign-off"),
        ("RRR", "Rogers"),
        ("RR73", "Rogers 73"),
    ]
}

fn print_help(program: &str) {
    eprintln!("FT8 Message Encoder");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  {}  \"message\"    Encode and show details for message", program);
    eprintln!("  {}  -T            Show examples of all message types", program);
    eprintln!("  {}  -h, --help    Show this help", program);
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  {}  \"CQ N0YPR DM42\"", program);
    eprintln!("  {}  \"N0YPR K1ABC RR73\"", program);
    eprintln!("  {}  -T", program);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Error: Missing argument");
        eprintln!();
        print_help(&args[0]);
        std::process::exit(1);
    }

    let arg = &args[1];

    match arg.as_str() {
        "-h" | "--help" => {
            print_help(&args[0]);
            std::process::exit(0);
        }
        "-T" => {
            // Show all message type examples
            println!("FT8 Message Type Examples");
            println!("=========================");
            println!();

            for (i, (msg, description)) in get_test_messages().iter().enumerate() {
                println!("Example {}: {} ({})", i + 1, msg, description);
                println!("{}", "-".repeat(60));

                match encode_message(msg) {
                    Ok(_) => {},
                    Err(e) => {
                        eprintln!("Error encoding '{}': {}", msg, e);
                    }
                }

                println!();
            }
        }
        message => {
            // Encode single message
            match encode_message(message) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}