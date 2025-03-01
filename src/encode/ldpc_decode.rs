use std::collections::{HashMap, HashSet};

use bitvec::prelude::*;

use crate::message::message::Message;

// reference: https://sourceforge.net/p/wsjt/wsjtx/ci/master/tree/lib/ft8/ldpc_174_91_c_generator.f90
// rather than a bunch of strings as in the fortran code, provide
// hex representations of each of the numbers. Drop the last bit
// since it is never used.
const GENERATOR_MATRIX: [u128; 83] = [
    0x8329ce11bf31eaf509f27fc >> 1,
    0x761c264e25c259335493132 >> 1,
    0xdc265902fb277c6410a1bdc >> 1,
    0x1b3f417858cd2dd33ec7f62 >> 1,
    0x09fda4fee04195fd034783a >> 1,
    0x077cccc11b8873ed5c3d48a >> 1,
    0x29b62afe3ca036f4fe1a9da >> 1,
    0x6054faf5f35d96d3b0c8c3e >> 1,
    0xe20798e4310eed27884ae90 >> 1,
    0x775c9c08e80e26ddae56318 >> 1,
    0xb0b811028c2bf997213487c >> 1,
    0x18a0c9231fc60adf5c5ea32 >> 1,
    0x76471e8302a0721e01b12b8 >> 1,
    0xffbccb80ca8341fafb47b2e >> 1,
    0x66a72a158f9325a2bf67170 >> 1,
    0xc4243689fe85b1c51363a18 >> 1,
    0x0dff739414d1a1b34b1c270 >> 1,
    0x15b48830636c8b99894972e >> 1,
    0x29a89c0d3de81d665489b0e >> 1,
    0x4f126f37fa51cbe61bd6b94 >> 1,
    0x99c47239d0d97d3c84e0940 >> 1,
    0x1919b75119765621bb4f1e8 >> 1,
    0x09db12d731faee0b86df6b8 >> 1,
    0x488fc33df43fbdeea4eafb4 >> 1,
    0x827423ee40b675f756eb5fe >> 1,
    0xabe197c484cb74757144a9a >> 1,
    0x2b500e4bc0ec5a6d2bdbdd0 >> 1,
    0xc474aa53d70218761669360 >> 1,
    0x8eba1a13db3390bd6718cec >> 1,
    0x753844673a27782cc42012e >> 1,
    0x06ff83a145c37035a5c1268 >> 1,
    0x3b37417858cc2dd33ec3f62 >> 1,
    0x9a4a5a28ee17ca9c324842c >> 1,
    0xbc29f465309c977e89610a4 >> 1,
    0x2663ae6ddf8b5ce2bb29488 >> 1,
    0x46f231efe457034c1814418 >> 1,
    0x3fb2ce85abe9b0c72e06fbe >> 1,
    0xde87481f282c153971a0a2e >> 1,
    0xfcd7ccf23c69fa99bba1412 >> 1,
    0xf0261447e9490ca8e474cec >> 1,
    0x4410115818196f95cdd7012 >> 1,
    0x088fc31df4bfbde2a4eafb4 >> 1,
    0xb8fef1b6307729fb0a078c0 >> 1,
    0x5afea7acccb77bbc9d99a90 >> 1,
    0x49a7016ac653f65ecdc9076 >> 1,
    0x1944d085be4e7da8d6cc7d0 >> 1,
    0x251f62adc4032f0ee714002 >> 1,
    0x56471f8702a0721e00b12b8 >> 1,
    0x2b8e4923f2dd51e2d537fa0 >> 1,
    0x6b550a40a66f4755de95c26 >> 1,
    0xa18ad28d4e27fe92a4f6c84 >> 1,
    0x10c2e586388cb82a3d80758 >> 1,
    0xef34a41817ee02133db2eb0 >> 1,
    0x7e9c0c54325a9c15836e000 >> 1,
    0x3693e572d1fde4cdf079e86 >> 1,
    0xbfb2cec5abe1b0c72e07fbe >> 1,
    0x7ee18230c583cccc57d4b08 >> 1,
    0xa066cb2fedafc9f52664126 >> 1,
    0xbb23725abc47cc5f4cc4cd2 >> 1,
    0xded9dba3bee40c59b5609b4 >> 1,
    0xd9a7016ac653e6decdc9036 >> 1,
    0x9ad46aed5f707f280ab5fc4 >> 1,
    0xe5921c77822587316d7d3c2 >> 1,
    0x4f14da8242a8b86dca73352 >> 1,
    0x8b8b507ad467d4441df770e >> 1,
    0x22831c9cf1169467ad04b68 >> 1,
    0x213b838fe2ae54c38ee7180 >> 1,
    0x5d926b6dd71f085181a4e12 >> 1,
    0x66ab79d4b29ee6e69509e56 >> 1,
    0x958148682d748a38dd68baa >> 1,
    0xb8ce020cf069c32a723ab14 >> 1,
    0xf4331d6d461607e95752746 >> 1,
    0x6da23ba424b9596133cf9c8 >> 1,
    0xa636bcbc7b30c5fbeae67fe >> 1,
    0x5cb0d86a07df654a9089a20 >> 1,
    0xf11f106848780fc9ecdd80a >> 1,
    0x1fbb5364fb8d2c9d730d5ba >> 1,
    0xfcb86bc70a50c9d02a5d034 >> 1,
    0xa534433029eac15f322e34c >> 1,
    0xc989d9c7c3d3b8c55d75130 >> 1,
    0x7bb38b2f0186d46643ae962 >> 1,
    0x2644ebadeb44b9467d1f42c >> 1,
    0x608cc857594bfbb55d69600 >> 1 ];

// https://sourceforge.net/p/wsjt/wsjtx/ci/master/tree/lib/ft8/ldpc_174_91_c_parity.f90
// Because it was fortran, all the indicies were 1 based, so subtracted 1 from each. 
// Because it was a one dimmensional array, needed to nest them
const FT8_TANNER_GRAPH_EDGES: [[usize; 3]; 174] = [
    [15, 44, 72],
    [24, 50, 61],
    [32, 57, 77],
    [0, 43, 44],
    [1, 6, 60],
    [2, 5, 53],
    [3, 34, 47],
    [4, 12, 20],
    [7, 55, 78],
    [8, 63, 68],
    [9, 18, 65],
    [10, 35, 59],
    [11, 36, 57],
    [13, 31, 42],
    [14, 62, 79],
    [16, 27, 76],
    [17, 73, 82],
    [21, 52, 80],
    [22, 29, 33],
    [23, 30, 39],
    [25, 40, 75],
    [26, 56, 69],
    [28, 48, 64],
    [2, 37, 77],
    [4, 38, 81],
    [45, 49, 72],
    [50, 51, 73],
    [54, 70, 71],
    [43, 66, 71],
    [42, 67, 77],
    [0, 31, 58],
    [1, 5, 70],
    [3, 15, 53],
    [6, 64, 66],
    [7, 29, 41],
    [8, 21, 30],
    [9, 17, 75],
    [10, 22, 81],
    [11, 27, 60],
    [12, 51, 78],
    [13, 49, 50],
    [14, 80, 82],
    [16, 28, 59],
    [18, 32, 63],
    [19, 25, 72],
    [20, 33, 39],
    [23, 26, 76],
    [24, 54, 57],
    [34, 52, 65],
    [35, 47, 67],
    [36, 45, 74],
    [37, 44, 46],
    [38, 56, 68],
    [40, 55, 61],
    [19, 48, 52],
    [45, 51, 62],
    [44, 69, 74],
    [26, 34, 79],
    [0, 14, 29],
    [1, 67, 79],
    [2, 35, 50],
    [3, 27, 50],
    [4, 30, 55],
    [5, 19, 36],
    [6, 39, 81],
    [7, 59, 68],
    [8, 9, 48],
    [10, 43, 56],
    [11, 38, 58],
    [12, 23, 54],
    [13, 20, 64],
    [15, 70, 77],
    [16, 29, 75],
    [17, 24, 79],
    [18, 60, 82],
    [21, 37, 76],
    [22, 40, 49],
    [6, 25, 57],
    [28, 31, 80],
    [32, 39, 72],
    [17, 33, 47],
    [12, 41, 63],
    [4, 25, 42],
    [46, 68, 71],
    [53, 54, 69],
    [44, 61, 67],
    [9, 62, 66],
    [13, 65, 71],
    [21, 59, 73],
    [34, 38, 78],
    [0, 45, 63],
    [0, 23, 65],
    [1, 4, 69],
    [2, 30, 64],
    [3, 48, 57],
    [0, 3, 4],
    [5, 59, 66],
    [6, 31, 74],
    [7, 47, 81],
    [8, 34, 40],
    [9, 38, 61],
    [10, 13, 60],
    [11, 70, 73],
    [12, 22, 77],
    [10, 34, 54],
    [14, 15, 78],
    [6, 8, 15],
    [16, 53, 62],
    [17, 49, 56],
    [18, 29, 46],
    [19, 63, 79],
    [20, 27, 68],
    [21, 24, 42],
    [12, 21, 36],
    [1, 46, 50],
    [22, 53, 73],
    [25, 33, 71],
    [26, 35, 36],
    [20, 35, 62],
    [28, 39, 43],
    [18, 25, 56],
    [2, 45, 81],
    [13, 14, 57],
    [32, 51, 52],
    [29, 42, 51],
    [5, 8, 51],
    [26, 32, 64],
    [24, 68, 72],
    [37, 54, 82],
    [19, 38, 76],
    [17, 28, 55],
    [31, 47, 70],
    [41, 50, 58],
    [27, 43, 78],
    [33, 59, 61],
    [30, 44, 60],
    [45, 67, 76],
    [5, 23, 75],
    [7, 9, 77],
    [39, 40, 69],
    [16, 49, 52],
    [41, 65, 67],
    [3, 21, 71],
    [35, 63, 80],
    [12, 28, 46],
    [1, 7, 80],
    [55, 66, 72],
    [4, 37, 49],
    [11, 37, 63],
    [58, 71, 79],
    [2, 25, 78],
    [44, 75, 80],
    [0, 64, 73],
    [6, 17, 76],
    [10, 55, 58],
    [13, 38, 53],
    [15, 36, 65],
    [9, 27, 54],
    [14, 59, 69],
    [16, 24, 81],
    [19, 29, 30],
    [11, 66, 67],
    [22, 74, 79],
    [26, 31, 61],
    [23, 68, 74],
    [18, 20, 70],
    [33, 52, 60],
    [34, 45, 46],
    [32, 58, 75],
    [39, 42, 82],
    [40, 41, 62],
    [48, 74, 82],
    [19, 43, 47],
    [41, 48, 56]
];

#[derive(Debug)]
struct Ft8_Ldpc {
    codeword: BitVec<u8, Msb0>,
    log_liklyhood_ratios: Vec<f64>,
    value_to_check: HashMap<usize, Vec<usize>>,
    check_to_value: HashMap<usize, Vec<usize>>,
}

impl Ft8_Ldpc {
    // pub fn from_message(mut message: Message) -> Result<Self, &'static str> {
    //     let message_and_checksum_bits = message.message_and_checksum_bits();

    //     let mut bitvec = BitVec::<u8, Msb0>::from_slice(message_and_checksum_bits);
    //     bitvec.split_off(91);

    //     println!("{:?}", bitvec);
        

    //     // let mut parity: u128 = 0;

    //     // for row in self.generator_matrix.iter() {
    //     //     parity = parity << 1;
    //     //     parity = parity | ((row & message).count_ones() % 2) as u128;
    //     // }

    //     for row in GENERATOR_MATRIX.iter() {
    //         //println!("{:b}", row);
    //         //110000010001100110010000101011101011001010010111111101110110101010111010110100101100000000
    //         // for i in (0..bitvec.len()).rev() {
    //         //     let bit = (row >> (127 - i)) & 1;
    //         // }
    //         // let bit = ((row & message))
    //     }


    //     return Err("");

    // }

    pub fn from_codeword(data: &[u8]) -> Result<Self, &'static str> {
        let mut codeword = BitVec::<u8, Msb0>::from_slice(data);
        if codeword.len() < 174 {
            return Err("Expected exactly 174 bits.");
        }
        if codeword.len() > 174 {
            let leftover_bits = codeword.split_off(174);    
            if leftover_bits.any() {
                return Err("Expected exactly 174 bits.");
            }
        }

        let log_liklyhood_ratios: Vec<f64> = vec![0.0; 174];

        let mut value_to_check: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut check_to_value: HashMap<usize, Vec<usize>> = HashMap::new();
        for (value_node, &check_nodes) in FT8_TANNER_GRAPH_EDGES.iter().enumerate() {
            // Insert into value-to-check map
            value_to_check.insert(value_node, check_nodes.to_vec());

            // Insert into check-to-value map
            for &check_node in &check_nodes {
                check_to_value.entry(check_node).or_insert_with(Vec::new).push(value_node);
            }
        }

        Ok(Ft8_Ldpc {
            codeword,
            log_liklyhood_ratios,
            value_to_check,
            check_to_value
        })
    }

    fn get_codeword_bit(&self, index: usize) -> u8 {
        if let Some(bit) = self.codeword.get(index) {
            if *bit { 1 } else { 0 }
        } else {
            0
        }
    }

    pub fn is_valid_codeword(&self) -> bool {        
        // Check if all value node sums are divisible by 2 (even) based on their corresponding check nodes.
        // If any sum is not divisible by 2, return `false`. If all sums are divisible by 2, return `true`.
        let valid_codeword = !self.check_to_value
            .values()
            .any(|value_nodes| {
                let sum: u8 = value_nodes
                    .iter()
                    .map(|i| self.get_codeword_bit(*i))
                    .sum();
                sum % 2 != 0
            });

        valid_codeword
    }


}

pub fn push_bits<T: Into<u128> + Copy>(bits: &mut BitVec<u8, Lsb0>, value: T, num_bits: usize) {
    let value:u128 = value.into();
    assert!(num_bits <= 128, "Cannot push more than 128 bits");
    for i in (0..num_bits).rev() {
        bits.push((value >> i) & 1 != 0);
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bitvec::slice::BitSliceIndex;

    use crate::encode::ldpc;

    use super::*;

    fn bitvec_from_str(s: &str) -> BitVec<u8, Msb0> {
        // Calculate the required padding length to make the string length a multiple of 8
        let len = s.len();
        let padding_len = (8 - len % 8) % 8; // ensures padding is between 0 and 7

        // Create the padded string
        let padded = format!("{:0>width$}", s, width = len + padding_len);

        // Convert the padded string into a BitVec
        let mut bitvec: BitVec<u8, Msb0> = padded.bytes().map(|b| b == b'1').collect();        
        while bitvec.leading_zeros() > 0 {
            bitvec.remove(0);
        }

        bitvec
    }

    fn raw_slice_from_str(s: &str) -> Vec<u8> {
        let bitvec = bitvec_from_str(s);
        bitvec.as_raw_slice().to_vec()
    }

    #[test]
    fn from_codeword_returns_ok_if_eq_174_bits() {
        let raw_slice = raw_slice_from_str("100101111100010101110001111100000101001001101100111001101000011001111100100010011001111111001011011010111110100010011011111100101111000111000100110110110101111100011100100011");
        let ldpc = Ft8_Ldpc::from_codeword(&raw_slice);
        assert!(ldpc.is_ok());
    }

    #[test]
    fn from_codeword_returns_err_if_lt_174_bits() {
        let raw_slice = raw_slice_from_str("1");
        let ldpc = Ft8_Ldpc::from_codeword(&raw_slice);
        assert!(ldpc.is_err());
    }

    #[test]
    fn from_codeword_returns_err_if_gt_174_bits() {
        let raw_slice = raw_slice_from_str("1100101111100010101110001111100000101001001101100111001101000011001111100100010011001111111001011011010111110100010011011111100101111000111000100110110110101111100011100100011");
        let ldpc = Ft8_Ldpc::from_codeword(&raw_slice);
        assert!(ldpc.is_err());
    }
    

    #[test]
    fn given_a_valid_codeword_is_valid_codeword_returns_true() {
        let raw_slice = raw_slice_from_str("100101111100010101110001111100000101001001101100111001101000011001111100100010011001111111001011011010111110100010011011111100101111000111000100110110110101111100011100100011");
        
        let ldpc = Ft8_Ldpc::from_codeword(&raw_slice).unwrap();
        
        assert!(ldpc.is_valid_codeword());
    }

    #[test]
    fn blah() {
        let mut bitvec = BitVec::<u8, Lsb0> ::new();
        bitvec.push(true);
        bitvec.push(false);
        bitvec.push(true);
        bitvec.push(true);
        println!("bitvec {:?}", bitvec);

        let raw_slice = bitvec.as_raw_slice();
        for b in raw_slice {
            print!("{:08b}", b);
        }
        println!();
        println!("raw_slice {:?}", raw_slice);

        let mut bitvec2 = BitVec::<u8, Lsb0>::from_slice(raw_slice);
        bitvec2.split_off(4);
        println!("bitvec2 {:?}", bitvec2);

        let n = 13u8;
        let mut bitvec3 = BitVec::<u8,Lsb0>::new();
        for i in (0..4) {
            let bit = (n >> i) & 1;
            bitvec3.push(bit == 1);
        }
        println!("bitvec3 {:?}", bitvec3);


        let b:u128 = 0x8329ce11bf31eaf509f27fc >> 1;
        let mut bitvec4 = BitVec::<u8,Lsb0>::new();
        for i in (0..90) {
            let bit = (b >> i) & 1;
            bitvec4.push(bit == 1);
        }
        println!("bitvec4 {:?}", bitvec4);

        let bitvec5: BitVec<u8, Lsb0> = (0..90)
            .map(|i| ((b >> i) & 1) == 1)
            .collect();
        println!("bitvec5 {:?}", bitvec5);
        //assert!(false);

    }

    #[test]
    fn blah2() {
        let message = Message::try_from("CQ SOTA N0YPR/R DM42").unwrap();
        let message_bits = message.bits();
        let crc_bits = message.checksum();
        let parity_bits:u128 = 0b111111111111111111111111111111111111111111111111111111111111111111111111111111111;

        let mut bitvec6: BitVec<u8, Lsb0> = BitVec::with_capacity(174);
        bitvec6.extend((0..77).rev().map(|i| ((message_bits >> i) & 1) == 1));
        bitvec6.extend((0..14).rev().map(|i| ((crc_bits >> i) & 1) == 1));
        bitvec6.extend((0..83).rev().map(|i| ((parity_bits >> i) & 1) == 1));

        println!("message_bits {:077b}", message_bits);
        println!("crc_bits {:014b}", crc_bits);
        println!("parity_bits {:083b}", parity_bits);
        println!("bitvec6 {:?}", bitvec6);


        let mut bitvec7: BitVec<u8, Lsb0> = BitVec::with_capacity(174);
        bitvec7.extend((0..83).rev().map(|i| ((parity_bits >> i) & 1) == 1));

        let slice = bitvec7.as_bitslice();
        println!("slice {:?}", slice);

        let mut bitvec8: BitVec<u8, Lsb0> = BitVec::with_capacity(174);
        push_bits(&mut bitvec8, message_bits, 77);
        push_bits(&mut bitvec8, crc_bits, 14);
        push_bits(&mut bitvec8, parity_bits, 83);
        let data = bitvec8.as_raw_slice();
        println!("data {:?}", data);
    }

    // #[test]
    // fn from_msg() {

    //     let a:u128 = 0x8329ce11bf31eaf509f27fc;
    //     let b:u128 = 0x8329ce11bf31eaf509f27fc >> 1;
    //     println!("{:b}", a);
    //     println!("{:b}", b);

    //     let mut message = Message::try_from("CQ SOTA N0YPR/R DM42").unwrap();
    //     let message_and_crc = message.message_and_checksum_bits();
        
    //     let mut message_bitvec = BitVec::<u8, Msb0>::from_slice(message_and_crc);
    //     if message_bitvec.len() > 91 {
    //         message_bitvec.split_off(91);
    //         //let leftover_bits = message_bitvec.split_off(91);    
    //         // if leftover_bits.any() {
    //         //     return Err("Expected exactly 174 bits.");
    //         // }
    //     }
    //     println!("{:?}", message_bitvec);


    //     let message_and_crc:u128 = 0b0000000001011110010110011000000001010010011011001110011011000110011111001000100001001100101;

    //     println!("{:b}", message_and_crc);

    //     let expected_parity_bits:u128 = 0b11100110011001101100100111100011101000010001100111111001100110001110011001011110010;


    //     for row in GENERATOR_MATRIX.iter() {
    //         println!("row: {:090b}", row);
    //         let mut row_bitvec = BitVec::<u8,Lsb0>::new();
    //         for i in (0..90).rev() {
    //             let bit = (row >> i) & 1;
    //             row_bitvec.push(bit == 1);
    //         }
    //         println!("row_bitvec: {:?}", row_bitvec);

            
    //         let parity_bit = (row & message_and_crc).count_ones() % 2;
    //         println!("{}", parity_bit);
    //     }
        



    //     //let ldpc = Ft8_Ldpc::from_message(message);

    // }
    
}
