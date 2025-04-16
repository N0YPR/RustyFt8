use crc::{Algorithm, Crc};

use crate::constants::CRC_POLYNOMIAL;

const CRC_FT8: Algorithm<u16> = Algorithm {
    width: 14,
    poly: CRC_POLYNOMIAL,
    init: 0x0,
    refin: false,
    refout: false,
    xorout: 0x0,
    check: 0x0,
    residue: 0x0
};

pub const FT8CRC: Crc<u16> = Crc::<u16>::new(&CRC_FT8);

pub fn checksum(msg:u128) -> u16 {
    // https://wsjt.sourceforge.io/FT4_FT8_QEX.pdf  page 8
    // "The CRC is calculated on the source-encoded message, zero-extended from 77 to 82 bits."
    let padded_msg = msg << 5;
    let msg_bytes = padded_msg.to_be_bytes();
    
    // Only need 11 of the bytes
    let trimmed_bytes = msg_bytes.as_slice()[msg_bytes.len()-11..].to_vec();

    let checksum = FT8CRC.checksum(&trimmed_bytes);
    return checksum;
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_ft8_crc() {
        let msg:u128 = 0b1110000111111100010100110101_0_1110001000000111101000011110_0_0_111001010001010_001;

        let c = checksum(msg);
        
        assert_eq!(c, 0b111100110010);
    }
}