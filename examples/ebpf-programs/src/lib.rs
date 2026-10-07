#![no_std]
#![no_builtins]

mod error;
mod netdev;
mod panic;

pub use error::{EbpfError, Verdict, dispatch};
pub use netdev::{try_from_netdev, try_to_netdev};
pub use panic::do_panic;
