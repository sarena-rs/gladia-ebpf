#![no_std]
#![no_builtins]

mod macros;
pub mod suite;
pub mod util;

pub use suite::TestSuite;
pub use waggle_macros::{act, arrange, assert};
pub use waggle_shared::*;

/// Not public API. Paths used by code generated from `waggle-macros`, so
/// users only need to depend on `waggle-ebpf`.
#[doc(hidden)]
pub mod __private {
    pub use aya_ebpf;
}
