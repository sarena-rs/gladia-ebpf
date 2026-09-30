#![no_std]
#![no_builtins]

mod macros;
pub mod suite;
pub mod util;

pub use suite::TestSuite;
pub use waggle_macros::{act, arrange, assert};
pub use waggle_shared::*;
