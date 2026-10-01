#![no_std]
#![no_main]
#![allow(nonstandard_style, dead_code)]

#[cfg(not(test))]
use ebpf_test_programs::do_panic;
use waggle_ebpf::{TestEntryCall, TestEntryHeader};

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    do_panic(info)
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";

// waggle::include_generated!();

// fn test() {
//     waggle::tail_call!("hello");
// }

#[used]
#[unsafe(link_section = ".test_entry_calls")]
static TEST_ENTRY_CALLS: TestEntryHeader<3> = TestEntryHeader {
    version: 1,
    file_name: make_name(b"main.rs"),
    count: 3u32,
    size: core::mem::size_of::<TestEntryCall>() as u32,
    entries: [
        TestEntryCall {
            index: 0u32,
            name: make_name(b"filter_ipv4"),
        },
        TestEntryCall {
            index: 1u32,
            name: make_name(b"filter_tcp"),
        },
        TestEntryCall {
            index: 2u32,
            name: make_name(b"filter_udp"),
        },
    ],
};

const fn make_name<const N: usize>(name: &[u8]) -> [u8; N] {
    let mut result = [0u8; N];

    // TODO: panic if name.len() > N
    let mut i = 0;
    while i < name.len() && i < N {
        result[i] = name[i];
        i += 1;
    }

    result
}
