//! Types and wire format shared by [gladia](https://crates.io/crates/gladia) and
//! [gladia-ebpf](https://crates.io/crates/gladia-ebpf).
//!
//! **Internal crate: depend on `gladia` or `gladia-ebpf` instead.** `gladia-ebpf` re-exports its
//! items. This crate has no stable API, and its version always matches theirs exactly.
//!
//! The `user` feature enables the userspace side: `std`, the `tlv_reader` module, and `aya` and
//! `serde` support for the shared types.

#![cfg_attr(not(feature = "user"), no_std)]
#![cfg_attr(not(feature = "user"), no_builtins)]
#![warn(missing_docs)]

pub mod constants;
pub mod scapy_assert;

#[cfg(feature = "user")]
pub mod tlv_reader;

pub mod tlv_writer;
pub mod wire;

pub use constants::*;
pub use scapy_assert::*;
pub use wire::*;

/// The size of a program name in a [`TestEntryCall`], including the terminating zero.
pub const STRING_SIZE: usize = 64;

/// The table of tail call targets that `gladia::build_mapping()` generates in the eBPF test
/// object, in the `.gladia_tail_call_section` section.
#[repr(C)]
pub struct TestEntryHeader<const N: usize> {
    /// The version of the table layout.
    pub version: u32,
    /// The size of one [`TestEntryCall`].
    pub size: u32,
    /// The number of entries.
    pub count: u32,
    /// The entries.
    pub entries: [TestEntryCall; N],
}

/// A production program that the tests tail call into.
#[repr(C)]
pub struct TestEntryCall {
    /// The slot of the program in the tail call map.
    pub index: u32,
    /// The name of the program, zero terminated.
    pub name: [u8; STRING_SIZE],
}
