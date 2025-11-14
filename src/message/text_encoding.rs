use alloc::string::String;
use alloc::format;

/// Encode text for c58 field (up to 10 characters) into 58 bits
/// Uses base-42 encoding with character set: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?'
pub fn encode_text_c58(text: &str) -> Result<u64, String> {
    const CHARSET: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";
    const BASE: u64 = 42;
    
    if text.len() > 10 {
        return Err(format!("c58 text must be 10 characters or less, got {}", text.len()));
    }
    
    // Right-align the text with spaces
    let padded = format!("{:>10}", text);
    
    // Encode using base-42: accumulator = accumulator * 42 + char_index
    let mut acc: u64 = 0;
    
    for ch in padded.bytes() {
        // Find character index in charset
        let idx = CHARSET.iter().position(|&c| c == ch)
            .ok_or_else(|| format!("Invalid character in c58 text: '{}'", ch as char))?;
        
        // Multiply accumulator by 42 and add index
        acc = acc * BASE + idx as u64;
    }
    
    Ok(acc)
}

/// Decode 58 bits back to text (10 characters)
pub fn decode_text_c58(value: u64) -> Result<String, String> {
    const CHARSET: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";
    const BASE: u64 = 42;
    
    let mut acc = value;
    let mut result = String::with_capacity(10);
    
    // Decode in reverse: extract character by dividing by 42
    for _ in 0..10 {
        let remainder = (acc % BASE) as usize;
        result.push(CHARSET[remainder] as char);
        acc /= BASE;
    }
    
    // Reverse since we decoded backwards
    Ok(result.chars().rev().collect())
}

/// Encode free text message (up to 13 characters) into 71 bits
/// Uses base-42 encoding with character set: ' 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?'
pub fn encode_free_text(text: &str) -> Result<[u8; 9], String> {
    const CHARSET: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";
    const BASE: u64 = 42;
    
    if text.len() > 13 {
        return Err(format!("Free text must be 13 characters or less, got {}", text.len()));
    }
    
    // Right-align the text with spaces (as per packtext77)
    let padded = format!("{:>13}", text);
    
    // Encode using base-42: accumulator = accumulator * 42 + char_index
    // We use a big-endian byte array to handle large numbers
    let mut acc = [0u8; 9];  // 71 bits = 9 bytes (7 bits + 8*8 bits)
    
    for ch in padded.bytes() {
        // Find character index in charset
        let idx = CHARSET.iter().position(|&c| c == ch)
            .ok_or_else(|| format!("Invalid character in free text: '{}'", ch as char))?;
        
        // Multiply accumulator by 42 and add index
        // acc = acc * 42 + idx
        multiply_add(&mut acc, BASE, idx as u64);
    }
    
    // Mask the first byte to only use 7 bits (clear MSB)
    acc[0] &= 0x7F;
    
    Ok(acc)
}

/// Decode 71 bits back to free text (13 characters)
pub fn decode_free_text(bits: &[u8; 9]) -> Result<String, String> {
    const CHARSET: &[u8] = b" 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";
    const BASE: u64 = 42;
    
    let mut acc = *bits;
    // Ensure first byte only uses 7 bits
    acc[0] &= 0x7F;
    
    let mut result = String::with_capacity(13);
    
    // Decode in reverse: extract character by dividing by 42
    for _ in 0..13 {
        let remainder = divide_inplace(&mut acc, BASE);
        result.push(CHARSET[remainder as usize] as char);
    }
    
    // Reverse since we decoded backwards
    Ok(result.chars().rev().collect())
}

/// Multiply a big-endian byte array by a value and add another value
/// Used for base-42 encoding
fn multiply_add(acc: &mut [u8; 9], multiplier: u64, addend: u64) {
    let mut carry = addend;
    
    // Process from least significant byte to most significant
    for i in (0..9).rev() {
        let val = (acc[i] as u64) * multiplier + carry;
        acc[i] = (val & 0xFF) as u8;
        carry = val >> 8;
    }
}

/// Divide a big-endian byte array by a value in place, returning the remainder
/// Used for base-42 decoding
fn divide_inplace(acc: &mut [u8; 9], divisor: u64) -> u64 {
    let mut remainder = 0u64;
    
    // Process from most significant byte to least significant
    for i in 0..9 {
        let val = (remainder << 8) | (acc[i] as u64);
        acc[i] = (val / divisor) as u8;
        remainder = val % divisor;
    }
    
    remainder
}
