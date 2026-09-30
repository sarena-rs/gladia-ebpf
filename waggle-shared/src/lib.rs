#![cfg_attr(not(feature = "std"), no_std)]
#![no_builtins]

pub mod constants;
pub mod scapy_assert;

#[cfg(feature = "std")]
pub mod tlv_reader;

pub mod tlv_writer;
pub mod wire;

pub use constants::*;
pub use scapy_assert::*;
pub use wire::*;

pub const STRING_SIZE: usize = 64;

#[repr(C)]
pub struct TestEntryHeader<const N: usize> {
    pub version: u32,
    pub file_name: [u8; STRING_SIZE],
    pub size: u32,
    pub count: u32,
    pub entries: [TestEntryCall; N],
}

#[repr(C)]
pub struct TestEntryCall {
    pub index: u32,
    pub name: [u8; STRING_SIZE],
}
