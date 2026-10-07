# gladia

A test framework for eBPF programs written with [Aya](https://aya-rs.dev).

Tests are eBPF programs written with
[gladia-ebpf](https://crates.io/crates/gladia-ebpf), in arrange/act/assert
style. This crate is the userspace runner. It loads your programs and their
tests, runs each test in the kernel with `BPF_PROG_TEST_RUN`, and reports the
results, including assertion failures and log messages from the eBPF side.

A project typically has three crates:

```text
my-programs/          # the production eBPF programs (no_std)
my-test-programs/     # the eBPF test programs (no_std), using gladia-ebpf
my-tests/             # a userspace crate with the test that runs them, using gladia
```

## In the eBPF test crate

`gladia` is a build dependency of the eBPF test crate, whose build script
generates the code for tail calling into the production programs by name. See
[gladia-ebpf](https://crates.io/crates/gladia-ebpf) for the setup.

```toml
[build-dependencies]
gladia = "0.1.0-alpha.1"
```

```rust
// my-test-programs/build.rs
fn main() {
    gladia::build_mapping();
}
```

## Running the tests

Add it as a dev-dependency of the userspace crate:

```toml
[dev-dependencies]
aya = "..."
gladia = "0.1.0-alpha.1"
```

The runner takes the two compiled eBPF objects as bytes: the production
programs and the test programs. How they are built is up to you, for example
with `aya-build` in a build script. Embed them with
`aya::include_bytes_aligned!`, so they can be parsed in place:

```rust
// my-tests/tests/ebpf.rs
static PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-programs"));
static TEST_PROGRAMS: &[u8] =
    aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-test-programs"));

#[test]
#[ignore = "requires root"]
fn ebpf() -> gladia::Res<()> {
    gladia::run_ebpf_test(PROGRAMS, TEST_PROGRAMS)
}
```

Loading eBPF programs requires root, or `CAP_BPF` and `CAP_NET_ADMIN`, so the
test is `#[ignore]`d and run explicitly, for example with
`sudo <test binary> --ignored --nocapture`. Each eBPF test prints its verdict
with its log messages, and the run ends with a summary:

```text
===== eBPF TEST SUMMARY =====
6 tests: 5 passed, 1 failed, 0 skipped

Failed:
    l2_announcement_arp_no_entry: test failed
```

The runner currently runs `tc` (`SchedClassifier`) programs only.

> **Status:** alpha; the API will change. This release is built on aya 0.14.
> When `assert_buffer!` finds a mismatch, the readable packet diff needs a
> Python script from the repository that is not part of this crate; without it,
> the test fails with an I/O error instead of the diff. A built-in packet
> verifier will replace it.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
