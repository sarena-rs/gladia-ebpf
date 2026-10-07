//! Procedural macros for [gladia-ebpf](https://crates.io/crates/gladia-ebpf).
//!
//! **Internal crate: depend on `gladia-ebpf` instead.** It re-exports these macros, and the code
//! they generate only compiles through it. This crate has no stable API, and its version always
//! matches `gladia-ebpf` exactly.

#![warn(missing_docs)]

mod act;
mod arrange;
mod assert;
mod common;
mod generate;

use proc_macro::TokenStream;

/// Marks the program that prepares the input of a test, for example by building the test packet.
///
/// Takes the program type (`tc` or `xdp`) and the name of the test. The function takes the
/// program context and returns a `TestStatus`; `TestStatus::Fail` or
/// `TestStatus::FrameworkError` fails the test before the `act` and `assert` programs run.
/// Optional.
///
/// ```rust,ignore
/// #[arrange(tc, "arp_request_is_answered")]
/// pub fn arp_request_arrange(ctx: TcContext) -> TestStatus {
///     let mut builder = PacketBuilder::new(&ctx);
///     builder.push_data(&ARP_REQUEST);
///     builder.build();
///     TestStatus::Pass
/// }
/// ```
#[proc_macro_attribute]
pub fn arrange(attrs: TokenStream, item: TokenStream) -> TokenStream {
    arrange::expand(attrs.into(), item.into()).into()
}

/// Marks the program that runs the code under test, usually by tail calling into a production
/// program with `tail_call!`.
///
/// Takes the program type (`tc` or `xdp`) and the name of the test. The function takes the
/// program context, and its return value, usually that of the production program, is stored in
/// the `TEST_SUITE_STATUS_CODE` map for the `assert` program. A return value equal to
/// `TestStatus::Fail` or `TestStatus::FrameworkError` fails the test before `assert` runs.
/// Optional.
///
/// ```rust,ignore
/// #[act(tc, "arp_request_is_answered")]
/// pub fn arp_request_act(ctx: TcContext) -> TestStatus {
///     tail_call!(&ctx, "from_netdev")
/// }
/// ```
#[proc_macro_attribute]
pub fn act(attrs: TokenStream, item: TokenStream) -> TokenStream {
    act::expand(attrs.into(), item.into()).into()
}

/// Marks the program that checks the result of a test and reports it to userspace.
///
/// Takes the program type (`tc` or `xdp`) and the name of the test. For `tc`, the function takes
/// the `TcContext` and a `&mut TestSuite`, which the assertion and logging macros of
/// `gladia-ebpf` write to. Every test needs one.
///
/// The function body is wrapped in a `loop`, which `test_fatal!`, `test_skip!` and a failing
/// `assert_test!` leave with `break` to end the test.
///
/// ```rust,ignore
/// #[assert(tc, "arp_request_is_answered")]
/// pub fn arp_request_assert(ctx: TcContext, t: &mut TestSuite) {
///     let len = ctx.data_end() - ctx.data();
///     assert_test!(t, len >= 42, "reply too short: %d bytes", len);
/// }
/// ```
#[proc_macro_attribute]
pub fn assert(attrs: TokenStream, item: TokenStream) -> TokenStream {
    assert::expand(attrs.into(), item.into()).into()
}

/// Includes the code that `gladia::build_mapping()` generated in the build script: the tail call
/// map and the `tail_call!` macro.
///
/// Use it at the top of the crate root, before the module declarations, so `tail_call!` is in
/// scope in every module.
///
/// ```rust,ignore
/// #![no_std]
///
/// gladia_ebpf::include_generated!();
///
/// mod arp;
/// ```
#[proc_macro]
pub fn include_generated(item: TokenStream) -> TokenStream {
    generate::expand(item.into()).into()
}
