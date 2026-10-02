#![no_std]
#![no_builtins]

mod macros;
pub mod suite;
pub mod util;

pub use gladia_macros::{act, arrange, assert, include_generated};
pub use gladia_shared::*;
pub use suite::TestSuite;

/// Not public API. Paths used by code generated from `gladia-macros`, so
/// users only need to depend on `gladia-ebpf`.
#[doc(hidden)]
pub mod __private {
    pub use aya_ebpf;
}
