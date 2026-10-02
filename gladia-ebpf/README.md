# gladia-ebpf

The eBPF side of [gladia](https://crates.io/crates/gladia), a test framework
for eBPF programs written with [Aya](https://aya-rs.dev).

Use this crate in the `no_std` crate that holds your eBPF test programs. Each
test has three programs, in arrange/act/assert style:

- **arrange**: prepares the input, for example by building the test packet.
- **act**: runs the program under test.
- **assert**: checks the result and reports it back to userspace.

```rust
use aya_ebpf::programs::TcContext;
use gladia_ebpf::{TestStatus, TestSuite, act, arrange, assert, assert_test};

#[arrange(tc, "drops_short_packets")]
pub fn drops_short_packets_arrange(ctx: TcContext) -> TestStatus {
    // Build the input packet.
    TestStatus::Pass
}

#[act(tc, "drops_short_packets")]
pub fn drops_short_packets_act(ctx: TcContext) -> TestStatus {
    // Call the program under test.
    TestStatus::Pass
}

#[assert(tc, "drops_short_packets")]
pub fn drops_short_packets_assert(ctx: TcContext, t: &mut TestSuite) {
    let len = ctx.data_end() - ctx.data();
    assert_test!(t, len >= 14, "packet too short: %d bytes", len);
}
```

Both `tc` and `xdp` programs are supported. The tests are loaded and run from
userspace with the `gladia` crate.

> **Status:** early development; the API will change.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
