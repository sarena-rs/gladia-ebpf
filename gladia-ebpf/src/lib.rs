//! The eBPF side of [gladia](https://crates.io/crates/gladia), a test framework for eBPF programs
//! written with [Aya](https://aya-rs.dev).
//!
//! Tests are eBPF programs themselves. They are compiled to BPF bytecode, pass the verifier, and
//! run in the kernel with `BPF_PROG_TEST_RUN`, against the exact bytecode of the production
//! programs. This crate is used in the `no_std` crate that holds those test programs; the
//! [`gladia`](https://docs.rs/gladia) crate loads and runs them from userspace.
//!
//! # Writing tests
//!
//! A test consists of up to three programs that share its name, in arrange/act/assert style:
//!
//! - [`macro@arrange`] (optional) prepares the input, for example by building the test packet;
//! - [`macro@act`] (optional) runs the production program under test, with `tail_call!`;
//! - [`macro@assert`] (required) checks the result and reports it to userspace.
//!
//! ```rust,ignore
//! use aya_ebpf::programs::TcContext;
//! use gladia_ebpf::{TestStatus, TestSuite, act, arrange, assert, assert_test, util::PacketBuilder};
//!
//! #[arrange(tc, "arp_request_is_answered")]
//! pub fn arp_request_arrange(ctx: TcContext) -> TestStatus {
//!     let mut builder = PacketBuilder::new(&ctx);
//!     builder.push_data(&ARP_REQUEST);
//!     builder.build();
//!     TestStatus::Pass
//! }
//!
//! #[act(tc, "arp_request_is_answered")]
//! pub fn arp_request_act(ctx: TcContext) -> TestStatus {
//!     // Runs the production program `from_netdev` on the packet.
//!     tail_call!(&ctx, "from_netdev")
//! }
//!
//! #[assert(tc, "arp_request_is_answered")]
//! pub fn arp_request_assert(ctx: TcContext, t: &mut TestSuite) {
//!     let len = ctx.data_end() - ctx.data();
//!     assert_test!(t, len >= 42, "reply too short: %d bytes", len);
//! }
//! ```
//!
//! The macros accept `tc` and `xdp` programs, but the `gladia` runner currently runs `tc` programs
//! only.
//!
//! # Setup
//!
//! The test crate depends on this crate and on the same `aya-ebpf` as this crate, and on `gladia`
//! as a build dependency:
//!
//! ```toml
//! [dependencies]
//! gladia-ebpf = "0.1"
//! aya-ebpf = "..."
//!
//! [build-dependencies]
//! gladia = "0.1"
//! ```
//!
//! Its build script calls `gladia::build_mapping()`, which finds every `tail_call!` in `src/`
//! and generates the tail call map and the `tail_call!` macro:
//!
//! ```rust,ignore
//! // build.rs
//! fn main() {
//!     gladia::build_mapping();
//! }
//! ```
//!
//! The crate root includes that generated code with [`include_generated!`], **before** its module
//! declarations, so `tail_call!` is in scope in every module:
//!
//! ```rust,ignore
//! // src/lib.rs
//! #![no_std]
//!
//! gladia_ebpf::include_generated!();
//!
//! mod arp;
//! ```
//!
//! `tail_call!(&ctx, "name")` jumps into the production program called `name`. A successful tail
//! call does not return; if it fails, the macro evaluates to [`TestStatus::FrameworkError`].
//!
//! # Assertions and logging
//!
//! Messages are static, printf-style format strings with integer arguments; they are formatted in
//! userspace. The supported specifiers are those of `bpf_trace_printk`: `%d`, `%u`, `%ld`, `%lu`,
//! `%lld`, `%llu`, `%x`, `%lx`, `%llx` and `%p`.
//!
//! - [`assert_test!`] fails the test, with an optional message, if a condition is false;
//! - [`test_log!`] logs a message;
//! - [`test_fatal!`] logs a message and fails the test;
//! - [`test_skip!`] skips the test;
//! - [`assert_buffer!`] compares packet bytes against an expected buffer.

#![no_std]
#![no_builtins]
#![warn(missing_docs)]

mod macros;
pub mod suite;
#[allow(missing_docs)]
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
