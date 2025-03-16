use bitvec::prelude::*;

pub fn bitvec_to_u128(bv: &BitVec, num_bits: usize) -> u128 {
    assert!(num_bits <= 128, "num_bits must be <= 128");
    let mut value = 0u128;
    for bit in &bv[0..num_bits] {
        value = (value << 1) | (*bit as u128);
    }
    value
}

pub fn bitslice_to_u128(bs: &BitSlice) -> u128 {
    assert!(bs.len() <= 128, "BitSlice length must be <= 128");
    let mut value = 0u128;
    for bit in bs {
        value = (value << 1) | (*bit as u128);
    }
    value
}

pub fn bitslice_to_u32(bs: &BitSlice) -> u32 {
    assert!(bs.len() <= 32, "BitSlice length must be <= 32");
    let mut value = 0u32;
    for bit in bs {
        value = (value << 1) | (*bit as u32);
    }
    value
}

pub fn bitslice_to_u8(bs: &BitSlice) -> u8 {
    assert!(bs.len() <= 8, "BitSlice length must be <= 8");
    let mut value = 0u8;
    for bit in bs {
        value = (value << 1) | (*bit as u8);
    }
    value
}

pub trait FromBitSlice {
    fn from_bitslice(bitslice: &BitSlice) -> Self;
}

impl FromBitSlice for u128 {
    fn from_bitslice(bitslice: &BitSlice) -> Self {
        assert!(bitslice.len() <= 128, "BitSlice length must be <= 128");
        let mut value = 0u128;
        for bit in bitslice {
            value = (value << 1) | (*bit as u128);
        }
        value
    }
}

impl FromBitSlice for u64 {
    fn from_bitslice(bitslice: &BitSlice) -> Self {
        assert!(bitslice.len() <= 64, "BitSlice length must be <= 64");
        let mut value = 0u64;
        for bit in bitslice {
            value = (value << 1) | (*bit as u64);
        }
        value
    }
}

impl FromBitSlice for u32 {
    fn from_bitslice(bitslice: &BitSlice) -> Self {
        assert!(bitslice.len() <= 32, "BitSlice length must be <= 32");
        let mut value = 0u32;
        for bit in bitslice {
            value = (value << 1) | (*bit as u32);
        }
        value
    }
}

impl FromBitSlice for u16 {
    fn from_bitslice(bitslice: &BitSlice) -> Self {
        assert!(bitslice.len() <= 16, "BitSlice length must be <= 16");
        let mut value = 0u16;
        for bit in bitslice {
            value = (value << 1) | (*bit as u16);
        }
        value
    }
}

impl FromBitSlice for u8 {
    fn from_bitslice(bitslice: &BitSlice) -> Self {
        assert!(bitslice.len() <= 8, "BitSlice length must be <= 8");
        let mut value = 0u8;
        for bit in bitslice {
            value = (value << 1) | (*bit as u8);
        }
        value
    }
}

pub trait BitvecToString {
    fn to_string(&mut self) -> String;
}
impl BitvecToString for BitVec {
    fn to_string(&mut self) -> String {
        self.iter().map(|b| if *b { '1' } else { '0' }).collect()
    }
}

pub trait PackBitvecFieldType {
    fn pack_into_bitvec(&self, bits: &mut BitVec, width: usize);
}

impl PackBitvecFieldType for bool {
    fn pack_into_bitvec(&self, bits: &mut BitVec, width: usize) {
        assert!(width == 1, "Width must be exactly 1");
        bits.push(*self);
    }
}

impl PackBitvecFieldType for u128 {
    fn pack_into_bitvec(&self, bits: &mut BitVec, width: usize) {
        assert!(width > 0, "Width must be at least 1");

        // Ensure that width does not exceed the size of the integer type
        assert!(width <= 128, "Width exceeds the bit size of the given type");
        
        for i in (0..width).rev() {
            bits.push(((*self) >> i) & 1 != 0);
        }
    }
}

impl PackBitvecFieldType for u64 {
    fn pack_into_bitvec(&self, bits: &mut BitVec, width: usize) {
        assert!(width <= 64, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}

impl PackBitvecFieldType for u32 {
    fn pack_into_bitvec(&self, bits: &mut BitVec, width: usize) {
        assert!(width <= 32, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}

impl PackBitvecFieldType for u16 {
    fn pack_into_bitvec(&self, bits: &mut BitVec, width: usize) {
        assert!(width <= 16, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}

impl PackBitvecFieldType for u8 {
    fn pack_into_bitvec(&self, bits: &mut BitVec, width: usize) {
        assert!(width <= 8, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}