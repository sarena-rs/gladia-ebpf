#![no_std]
#![no_main]
#![allow(nonstandard_style, dead_code)]

use aya_ebpf::programs::TcContext;
#[cfg(not(test))]
use ebpf_test_programs::do_panic;
use ebpf_test_programs::tail_call;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    do_panic(info)
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";

fn test(ctx: TcContext) {
    tail_call!(&ctx, "hello");
    tail_call!(&ctx, "world");
    tail_call!(&ctx, "hello");

    tail_call!(&ctx, "bladiebla");
}

// To expand macros:
// cargo +nightly expand --manifest-path examples/Cargo.toml -p ebpf-test-programs --bin
// ebpf-test-programs
