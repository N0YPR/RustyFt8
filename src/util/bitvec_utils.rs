use bitvec::prelude::*;

pub trait AlignBitvec {
    fn align_right(&mut self);
}
impl AlignBitvec for BitVec<u8, Msb0> {
    fn align_right(&mut self) {
        while self.len() % 8 > 0 {
            self.insert(0, false);
        }
    }
}

pub trait PackBitvecFieldType {
    fn pack_into_bitvec(&self, bits: &mut BitVec<u8, Msb0>, width: usize);
}

impl PackBitvecFieldType for bool {
    fn pack_into_bitvec(&self, bits: &mut BitVec<u8, Msb0>, width: usize) {
        assert!(width == 1, "Width must be exactly 1");
        bits.push(*self);
    }
}

impl PackBitvecFieldType for u128 {
    fn pack_into_bitvec(&self, bits: &mut BitVec<u8, Msb0>, width: usize) {
        assert!(width > 0, "Width must be at least 1");

        // Ensure that width does not exceed the size of the integer type
        assert!(width <= 128, "Width exceeds the bit size of the given type");
        
        for i in (0..width).rev() {
            bits.push(((*self) >> i) & 1 != 0);
        }
    }
}

impl PackBitvecFieldType for u64 {
    fn pack_into_bitvec(&self, bits: &mut BitVec<u8, Msb0>, width: usize) {
        assert!(width <= 64, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}

impl PackBitvecFieldType for u32 {
    fn pack_into_bitvec(&self, bits: &mut BitVec<u8, Msb0>, width: usize) {
        assert!(width <= 32, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}

impl PackBitvecFieldType for u16 {
    fn pack_into_bitvec(&self, bits: &mut BitVec<u8, Msb0>, width: usize) {
        assert!(width <= 16, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}

impl PackBitvecFieldType for u8 {
    fn pack_into_bitvec(&self, bits: &mut BitVec<u8, Msb0>, width: usize) {
        assert!(width <= 8, "Width exceeds the bit size of the given type");
        let field:u128 = (*self).into();
        field.pack_into_bitvec(bits, width);
    }
}