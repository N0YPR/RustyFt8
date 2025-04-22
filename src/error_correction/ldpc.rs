#![allow(unused)]

use bitvec::prelude::*;

use crate::constants::{FT8_LDPC_CHECK_TO_VALUE, FT8_LDPC_GENERATOR_MATRIX};
use crate::util::bitvec_utils::{bitvec_to_u128, FromBitSlice, PackBitvecFieldType};

use super::checksum::checksum;

#[derive(Debug)]
pub struct Ft8Ldpc {
    codeword: BitVec,
    log_liklyhood_ratios: Vec<f64>,
}

impl Ft8Ldpc {
    pub fn from_message(message: u128) -> Self {
        let mut codeword = BitVec::new();
        message.pack_into_bitvec(&mut codeword, 77);

        let crc = checksum(message);
        crc.pack_into_bitvec(&mut codeword, 14);

        let parity = generate_parity(message, crc);
        parity.pack_into_bitvec(&mut codeword, 83);

        let log_liklyhood_ratios: Vec<f64> = codeword
            .iter()
            .map(|bit| if *bit { 1.0 } else { -1.0 })
            .collect();

        Ft8Ldpc {
            codeword,
            log_liklyhood_ratios,
        }
    }

    pub fn from_codeword_bits(codeword_bits: &BitSlice) -> Result<Self, &'static str> {
        if codeword_bits.len() != 174 {
            return Err("codeword_bits must be exactly 174 bits long");
        }

        let codeword: BitVec = codeword_bits.to_bitvec();
        let log_liklyhood_ratios: Vec<f64> = codeword
            .iter()
            .map(|bit| if *bit { 1.0 } else { -1.0 })
            .collect();

        Ok(Ft8Ldpc {
            codeword,
            log_liklyhood_ratios,
        })
    }

    pub fn from_log_likelihood_ratios(log_liklyhood_ratios: &[f64]) -> Result<Self, &'static str> {
        if log_liklyhood_ratios.len() != 174 {
            return Err("log_liklyhood_ratios must be exactly 174 elements long");
        }

        let codeword: BitVec = log_liklyhood_ratios.iter().map(|&llr| llr > 0.0).collect();

        Ok(Ft8Ldpc {
            codeword,
            log_liklyhood_ratios: log_liklyhood_ratios.to_vec(),
        })
    }

    pub fn get_codeword_bits(&self) -> &BitSlice {
        &self.codeword
    }

    pub fn get_message(&self) -> u128 {
        u128::from_bitslice(&self.codeword[0..77])
    }

    pub fn get_crc(&self) -> u16 {
        u16::from_bitslice(&self.codeword[77..91])
    }

    pub fn get_parity(&self) -> u128 {
        u128::from_bitslice(&self.codeword[91..174])
    }

    pub fn get_codeword_bit(&self, index: usize) -> u8 {
        if let Some(bit) = self.codeword.get(index) {
            if *bit {
                1
            } else {
                0
            }
        } else {
            0
        }
    }

    pub fn get_log_likelihood_ratios(&self) -> &[f64] {
        &self.log_liklyhood_ratios
    }

    pub fn is_valid(&self) -> bool {
        // Check if all value node sums are divisible by 2 (even) based on their corresponding check nodes.
        // If any sum is not divisible by 2, return `false`.
        for value_nodes in FT8_LDPC_CHECK_TO_VALUE.iter() {
            let sum: u8 = value_nodes.iter().map(|&i| self.get_codeword_bit(i)).sum();
            if sum % 2 != 0 {
                return false;
            }
        }

        // re-check the crc
        let msg = self.get_message();
        let recalculated_crc = checksum(msg);
        return recalculated_crc == self.get_crc();
    }

    pub fn solve(&mut self) {
        let max_iterations = 50;
        let mut messages = vec![vec![0.0; FT8_LDPC_CHECK_TO_VALUE.len()]; self.codeword.len()];

        for _ in 0..max_iterations {
            // Variable nodes to check nodes
            for (i, &llr) in self.log_liklyhood_ratios.iter().enumerate() {
                for (j, check_nodes) in FT8_LDPC_CHECK_TO_VALUE.iter().enumerate() {
                    let mut message = llr;
                    for &k in *check_nodes {
                        if k != i {
                            message += messages[k][j];
                        }
                    }
                    messages[i][j] = message;
                }
            }

            // Check nodes to variable nodes
            for (j, check_nodes) in FT8_LDPC_CHECK_TO_VALUE.iter().enumerate() {
                for &i in *check_nodes {
                    let mut product = 1.0;
                    for &k in *check_nodes {
                        if k != i {
                            product *= (1.0 - 2.0 * messages[k][j].tanh());
                        }
                    }
                    messages[i][j] = 0.5 * (1.0 + product).ln();
                }
            }

            // Update beliefs
            let original_llrs = self.log_liklyhood_ratios.clone();
            for (i, llr) in self.log_liklyhood_ratios.iter_mut().enumerate() {
                *llr = original_llrs[i];
                for (j, check_nodes) in FT8_LDPC_CHECK_TO_VALUE.iter().enumerate() {
                    if check_nodes.contains(&i) {
                        *llr += messages[i][j];
                    }
                }
            }
        }

        // Set the codeword from updated LLRs
        for (i, &llr) in self.log_liklyhood_ratios.iter().enumerate() {
            self.codeword.set(i, llr > 0.0);
        }
    }
}

pub fn generate_parity(message: u128, crc: u16) -> u128 {
    let mut bv: BitVec = BitVec::new();
    message.pack_into_bitvec(&mut bv, 77);
    crc.pack_into_bitvec(&mut bv, 14);
    let message_and_crc = bitvec_to_u128(&bv, 91);

    let mut parity: u128 = 0;

    for row in FT8_LDPC_GENERATOR_MATRIX.iter() {
        parity = parity << 1;
        parity = parity | ((row & message_and_crc).count_ones() % 2) as u128;
    }

    parity
}

#[cfg(test)]
mod tests {
    use super::*;

    mod from_message {
        use std::sync::LazyLock;

        use super::*;

        static CODEWORD: LazyLock<Ft8_Ldpc> = LazyLock::new(|| {
            Ft8_Ldpc::from_message(0b0000000001011110010110011000_0_0000101001001101100111001101_1_0_001100111110011_001)
        });

        #[test]
        fn message_is_correct() {
            let message = CODEWORD.get_message();
            assert_eq!(message, 0b0000000001011110010110011000_0_0000101001001101100111001101_1_0_001100111110011_001);
        }

        #[test]
        fn crc_is_correct() {
            let crc = CODEWORD.get_crc();
            assert_eq!(crc, 0b0011101111001110);
        }

        #[test]
        fn parity_is_correct() {
            let parity = CODEWORD.get_parity();
            assert_eq!(
                parity,
                0b100000011011011111000110110110110011110000100001110111111100101100010100010011111
            );
        }

        #[test]
        fn get_log_likelihood_ratios_correct() {
            let codeword_bits = CODEWORD.get_codeword_bits();
            let log_liklyhood_ratios = CODEWORD.get_log_likelihood_ratios();
            for (bit, &llr) in codeword_bits.iter().zip(log_liklyhood_ratios.iter()) {
                if *bit {
                    assert!(llr > 0.0);
                } else {
                    assert!(llr < 0.0);
                }
            }
        }

        #[test]
        fn codeword_is_valid() {
            let is_valid = CODEWORD.is_valid();
            assert!(is_valid);
        }
    }

    mod from_valid_codeword_bits {
        use std::sync::LazyLock;

        use super::*;

        static CODEWORD: LazyLock<Ft8_Ldpc> = LazyLock::new(|| {
            Ft8_Ldpc::from_codeword_bits(&bits![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 0, 1, 1, 0, 0, 0,
                0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 0,
                1, 1, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1, 1, 1, 0, 1, 1, 1,
                1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 1, 1, 0,
                0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0,
                1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0,
                0, 1, 1, 1, 1, 1
            ])
            .unwrap()
        });

        #[test]
        fn message_is_correct() {
            let message = CODEWORD.get_message();
            assert_eq!(message, 0b0000000001011110010110011000_0_0000101001001101100111001101_1_0_001100111110011_001);
        }

        #[test]
        fn crc_is_correct() {
            let crc = CODEWORD.get_crc();
            assert_eq!(crc, 0b0011101111001110);
        }

        #[test]
        fn parity_is_correct() {
            let parity = CODEWORD.get_parity();
            assert_eq!(
                parity,
                0b100000011011011111000110110110110011110000100001110111111100101100010100010011111
            );
        }

        #[test]
        fn get_log_likelihood_ratios_correct() {
            let codeword_bits = CODEWORD.get_codeword_bits();
            let log_liklyhood_ratios = CODEWORD.get_log_likelihood_ratios();
            for (bit, &llr) in codeword_bits.iter().zip(log_liklyhood_ratios.iter()) {
                if *bit {
                    assert!(llr > 0.0);
                } else {
                    assert!(llr < 0.0);
                }
            }
        }

        #[test]
        fn codeword_is_valid() {
            let is_valid = CODEWORD.is_valid();
            assert!(is_valid);
        }
    }

    #[test]
    fn from_empty_codeword_bits_returns_err() {
        assert!(Ft8_Ldpc::from_codeword_bits(&bits![]).is_err());
    }

    #[test]
    fn from_too_many_codeword_bits_returns_err() {
        assert!(Ft8_Ldpc::from_codeword_bits(&bitvec![0; 200]).is_err());
    }

    mod from_valid_log_liklyhood_ratios {
        use std::sync::LazyLock;

        use super::*;

        static CODEWORD: LazyLock<Ft8_Ldpc> = LazyLock::new(|| {
            Ft8_Ldpc::from_log_likelihood_ratios(&vec![
                -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, 1.0,
                1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0,
                -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
                -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
                -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
                1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
                1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0,
                -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0,
                1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0,
                -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0,
                1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0,
                1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
            ])
            .unwrap()
        });

        #[test]
        fn message_is_correct() {
            let message = CODEWORD.get_message();
            assert_eq!(message, 0b0000000001011110010110011000_0_0000101001001101100111001101_1_0_001100111110011_001);
        }

        #[test]
        fn crc_is_correct() {
            let crc = CODEWORD.get_crc();
            assert_eq!(crc, 0b0011101111001110);
        }

        #[test]
        fn parity_is_correct() {
            let parity = CODEWORD.get_parity();
            assert_eq!(
                parity,
                0b100000011011011111000110110110110011110000100001110111111100101100010100010011111
            );
        }

        #[test]
        fn get_log_likelihood_ratios_correct() {
            let codeword_bits = CODEWORD.get_codeword_bits();
            let log_liklyhood_ratios = CODEWORD.get_log_likelihood_ratios();
            for (bit, &llr) in codeword_bits.iter().zip(log_liklyhood_ratios.iter()) {
                if *bit {
                    assert!(llr > 0.0);
                } else {
                    assert!(llr < 0.0);
                }
            }
        }

        #[test]
        fn codeword_is_valid() {
            let is_valid = CODEWORD.is_valid();
            assert!(is_valid);
        }
    }

    #[test]
    fn from_empty_log_likelihood_ratios_returns_err() {
        assert!(Ft8_Ldpc::from_log_likelihood_ratios(&vec![]).is_err());
    }

    #[test]
    fn from_too_many_log_likelihood_ratios_returns_err() {
        assert!(Ft8_Ldpc::from_log_likelihood_ratios(&vec![0.0; 200]).is_err());
    }

    #[test]
    fn from_invalid_log_likelihood_ratios_is_valid_returns_false() {
        let codeword = Ft8_Ldpc::from_log_likelihood_ratios(&vec![
            1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0,
            -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0,
            -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0,
            1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0,
            1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
            1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0,
            1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0,
            1.0, 1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
            -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, -1.0, -1.0,
            1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0,
            1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
            1.0, 1.0, 1.0,
        ])
        .unwrap();
        assert!(!codeword.is_valid());
    }

    #[test]
    fn from_invalid_log_likelihood_ratios_solve() {
        let mut codeword = Ft8_Ldpc::from_log_likelihood_ratios(&vec![
            1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0,
            -1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0,
            -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0,
            1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, -1.0,
            1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0,
            1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0,
            1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0,
            1.0, 1.0, -1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0,
            -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, -1.0, -1.0,
            1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0,
            1.0, -1.0, -1.0, -1.0, 1.0, -1.0, 1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0,
            1.0, 1.0, 1.0,
        ])
        .unwrap();
        assert!(!codeword.is_valid());
        codeword.solve();
        assert!(codeword.is_valid());
    }

    #[test]
    fn when_crc_is_not_valid_codeword_is_valid_returns_false() {
        let msg: u128 =
            0b0000000001011110010110011000_0_0000101001001101100111001101_1_0_001100111110011_001;

        let mut codeword = BitVec::new();
        msg.pack_into_bitvec(&mut codeword, 77);

        let crc = checksum(msg);
        let invalid_crc = crc ^ 0b1;
        invalid_crc.pack_into_bitvec(&mut codeword, 14);

        let parity = generate_parity(msg, invalid_crc);
        parity.pack_into_bitvec(&mut codeword, 83);

        let log_liklyhood_ratios: Vec<f64> = codeword
            .iter()
            .map(|bit| if *bit { 1.0 } else { -1.0 })
            .collect();

        let codeword = Ft8_Ldpc {
            codeword,
            log_liklyhood_ratios,
        };

        assert!(!codeword.is_valid())
    }
}
