# gladia-ebpf

The eBPF side of [gladia](https://crates.io/crates/gladia), a test framework
for eBPF programs written with [Aya](https://aya-rs.dev).

Tests are eBPF programs themselves: they run in the kernel with
`BPF_PROG_TEST_RUN`, against the exact bytecode of the production programs.
Use this crate in the `no_std` crate that holds those test programs. Each test
has up to three programs, in arrange/act/assert style:

- **arrange** (optional): prepares the input, for example by building the test
  packet.
- **act** (optional): runs the production program under test, by tail calling
  into it.
- **assert**: checks the result and reports it back to userspace.

## Setup

Depend on this crate and on the same `aya-ebpf` as this crate, and on `gladia`
as a build dependency:

```toml
[dependencies]
gladia-ebpf = "0.1"
aya-ebpf = "..."

[build-dependencies]
gladia = "0.1"
```

The build script generates the code for tail calling into the production
programs by name:

```rust
// build.rs
fn main() {
    gladia::build_mapping();
}
```

Include that generated code at the top of the crate root, **before** the module
declarations, so the `tail_call!` macro is in scope in every module:

```rust
// src/lib.rs
#![no_std]

gladia_ebpf::include_generated!();

mod arp;
```

Like any eBPF crate, it is built with a nightly toolchain (`-Z build-std=core`)
and [`bpf-linker`](https://github.com/aya-rs/bpf-linker).

## Writing tests

```rust
use aya_ebpf::programs::TcContext;
use gladia_ebpf::{TestStatus, TestSuite, act, arrange, assert, assert_test, util::PacketBuilder};

#[arrange(tc, "arp_request_is_answered")]
pub fn arp_request_arrange(ctx: TcContext) -> TestStatus {
    let mut builder = PacketBuilder::new(&ctx);
    builder.push_data(&ARP_REQUEST);
    builder.build();
    TestStatus::Pass
}

#[act(tc, "arp_request_is_answered")]
pub fn arp_request_act(ctx: TcContext) -> TestStatus {
    // Runs the production program `from_netdev` on the packet.
    tail_call!(&ctx, "from_netdev")
}

#[assert(tc, "arp_request_is_answered")]
pub fn arp_request_assert(ctx: TcContext, t: &mut TestSuite) {
    let len = ctx.data_end() - ctx.data();
    assert_test!(t, len >= 42, "reply too short: %d bytes", len);
}
```

The assertion and logging macros (`assert_test!`, `test_log!`, `test_fatal!`,
`test_skip!` and `assert_buffer!`) take static, printf-style format strings with
integer arguments, which are formatted in userspace.

The macros accept `tc` and `xdp` programs, but the `gladia` runner currently
runs `tc` programs only.

The tests are loaded and run from userspace with the
[gladia](https://crates.io/crates/gladia) crate.

> **Status:** early development; the API will change.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
