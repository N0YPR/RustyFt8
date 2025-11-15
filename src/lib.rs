#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod crc;
pub mod ldpc;
pub mod message;
pub mod symbol;
pub mod pulse;
pub mod wav;
pub mod sync;

pub use message::{encode, decode};
