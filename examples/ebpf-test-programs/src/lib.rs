#![no_std]
#![no_builtins]

// Must come before the modules so `tail_call!` is in scope inside them.
gladia_ebpf::include_generated!();

mod arp;
mod dummy;
mod panic;
mod prod_calls;
mod scapy_bytes;
mod scapy_tests;

pub use panic::do_panic;
