# Gladia eBPF: an eBPF test framework (TCX/XDP)

[![ci](https://github.com/sarena-rs/gladia-ebpf/actions/workflows/ci.yaml/badge.svg)](https://github.com/sarena-rs/gladia-ebpf/actions/workflows/ci.yaml)
[![made-with-rust](https://img.shields.io/badge/Made%20with-Rust-1f425f.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/github/license/sarena-rs/gladia-ebpf.svg)](https://github.com/sarena-rs/gladia-ebpf/blob/master/LICENSE-APACHE)

Gladia is a framework for testing eBPF programs 🐝 written in Rust 🦀 with [Aya](https://aya-rs.dev),
where they actually run: as BPF bytecode, in the kernel, after passing the verifier.

> **Status: work in progress.** The framework is usable, but young. The API, the generated code
> and the wire format between the eBPF side and userspace will still change, and some parts are
> tied to the layout of this repository. See [Limitations and future work](#limitations-and-future-work).

## Introduction

Ordinary Rust unit tests do not get you far with eBPF code:

- Code that compiles and passes the verifier can still behave differently in the kernel than a
  userspace build of the same functions.
- eBPF intrinsics such as maps, helpers and tail calls are hard to emulate faithfully.
- Userspace cannot construct a real execution context, such as an `__sk_buff` with its packet
  boundaries and memory layout.

gladia therefore runs the tests themselves as eBPF programs. Each test consists of up to three
programs, in the classic **arrange / act / assert** shape:

- **arrange** builds the input, for example a test packet;
- **act** runs the production program under test, by tail calling into the real, compiled
  production object;
- **assert** checks the result and reports back to userspace.

A userspace runner loads the production and test objects, wires them together, runs every test
in the kernel with `BPF_PROG_TEST_RUN`, and prints the results, including the log messages and
assertion failures written by the eBPF side. Only this orchestration happens in userspace; the
code under test is the exact bytecode that is deployed.

The approach is inspired by [Cilium's BPF unit test framework](https://github.com/cilium/cilium),
reimplemented in Rust on top of Aya. The background is described in the blog post
[Testing eBPF code where it actually runs](https://erwinkok.org/posts/ebpf-testing-framework/), and
the internals in the [design document](docs/design.md).

## Crates

| Crate | Used in | Purpose |
|---|---|---|
| [`gladia-ebpf`](gladia-ebpf) | your eBPF test programs (`no_std`) | the `#[arrange]`, `#[act]` and `#[assert]` macros, assertions, logging and packet helpers |
| [`gladia`](gladia) | your userspace test, and the `build.rs` of your eBPF test programs | the test runner, and the code generation for tail calls |
| [`gladia-macros`](gladia-macros) | internal | the procedural macros, re-exported by `gladia-ebpf` |
| [`gladia-shared`](gladia-shared) | internal | types and wire format shared by both sides |

## Getting started

### Prerequisites

- A nightly Rust toolchain with `rust-src`: eBPF crates are built with `-Z build-std=core`. This
  repository pins one in `rust-toolchain.toml`.
- [`bpf-linker`](https://github.com/aya-rs/bpf-linker): `cargo install bpf-linker`.
- Linux with BTF, and root (or `CAP_BPF` and `CAP_NET_ADMIN`) to load the programs.
- Optional: Python 3 with [scapy](https://scapy.net), for readable packet diffs when a buffer
  assertion fails (see [Packet assertions](#packet-assertions)).

### Project layout

A project typically has three crates:

```text
my-programs/          # the production eBPF programs (no_std)
my-test-programs/     # the eBPF test programs (no_std), using gladia-ebpf
my-tests/             # a userspace crate with the test that runs them, using gladia
```

The production programs need no changes to be tested: the tests call into them by name.

### Writing tests

In the eBPF test crate, depend on `gladia-ebpf`, and on `gladia` as a build dependency:

```toml
# my-test-programs/Cargo.toml
[dependencies]
gladia-ebpf = "0.1"
aya-ebpf = "..."

[build-dependencies]
gladia = "0.1"
```

Its build script generates the code for calling the production programs:

```rust
// my-test-programs/build.rs
fn main() {
    gladia::build_mapping();
}
```

Include that generated code at the top of the crate root, **before** the module declarations, so
the `tail_call!` macro is in scope in every module:

```rust
// my-test-programs/src/lib.rs
#![no_std]

gladia_ebpf::include_generated!();

mod arp;
```

Then write the tests. The three programs of a test share its name; `arrange` and `act` are
optional, `assert` is required:

```rust
// my-test-programs/src/arp.rs
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

`tail_call!(&ctx, "name")` jumps into the production program called `name`. A successful tail
call does not return; if it fails, the macro evaluates to `TestStatus::FrameworkError`. The names
are collected by the build script, so any production program can be called without registering
it anywhere.

### Assertions and logging

eBPF programs have no allocator, so messages are static, printf-style format strings with up to a
handful of integer arguments. The formatting happens in userspace.

| Macro | Purpose |
|---|---|
| `assert_test!(t, cond)`, `assert_test!(t, cond, "fmt %d", arg)` | fail the test, with a message, if `cond` is false |
| `test_log!(t, "fmt %llu", args...)` | log a message |
| `test_fatal!(t, "fmt", args...)` | log a message and fail the test |
| `test_skip!(t)` | skip the test |
| `assert_buffer!(t, ctx, "name", "Ether", offset, buf, len)` | compare packet bytes against an expected buffer |

Supported specifiers are those of `bpf_trace_printk`: `%d`, `%u`, `%ld`, `%lu`, `%lld`, `%llu`,
`%x`, `%lx`, `%llx` and `%p`.

### Building packets

`util::PacketBuilder` writes the test packet into the context in an `arrange` program. For now it
copies prepared bytes into the packet (`push_data`), for example a packet captured or generated
with scapy. Building packets layer by layer is planned; see
[Limitations and future work](#limitations-and-future-work).

### Packet assertions

When `assert_buffer!` finds a mismatch, the runner passes both packets to
`scapy/trace_diff_pkts.py`, which decodes them with scapy (starting at the given first layer, such
as `"Ether"`) and prints a field-by-field diff. This needs a Python environment with scapy in
`scapyenv/`:

```sh
python3 -m venv scapyenv
scapyenv/bin/pip install -r scapy/requirements.txt
```

### Running the tests

The runner takes the two compiled eBPF objects as bytes: the production programs and the test
programs that exercise them.

```rust
gladia::run_ebpf_test(PROGRAMS, TEST_PROGRAMS)
```

How the objects are built is up to you; gladia does not impose a build strategy, toolchain or
compiler settings. Embed the objects with `aya::include_bytes_aligned!`, so they can be parsed in
place.

#### With aya-build

Build the eBPF packages from the `build.rs` of the crate that holds the test. The objects are
placed in `OUT_DIR`:

```rust
// my-tests/build.rs
use aya_build::{Package, Toolchain};

fn main() {
    let packages = [
        Package { name: "my-programs", root_dir: "../my-programs", ..Default::default() },
        Package { name: "my-test-programs", root_dir: "../my-test-programs", ..Default::default() },
    ];
    aya_build::build_ebpf(packages, Toolchain::default()).unwrap();
}
```

```rust
// my-tests/tests/ebpf.rs
static PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-programs"));
static TEST_PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-test-programs"));

#[test]
#[ignore = "requires root"]
fn ebpf() -> gladia::Res<()> {
    gladia::run_ebpf_test(PROGRAMS, TEST_PROGRAMS)
}
```

#### With a separate build step

Build the objects beforehand (with an xtask, a script, ...) and point the test at them. This
repository does so for `examples/`: `just build-ebpf` writes the objects to `target/ebpf-objects`,
and `.cargo/config.toml` sets `EBPF_OBJECTS` to that directory:

```rust
static PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("EBPF_OBJECTS"), "/ebpf-programs.o"));
static TEST_PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("EBPF_OBJECTS"), "/ebpf-test-programs.o"));
```

Here, rebuilding the objects after changing the eBPF code is up to that build step.

#### Output

Because loading requires root, the test is `#[ignore]`d and run explicitly, for example with
`sudo <test binary> --ignored --nocapture`. Each eBPF test prints its verdict
(`[PASS]`, `[FAIL]`, `[SKIP]` or `[FRAMEWORK ERROR]`) with its log messages, and the run ends with
a summary:

```text
===== eBPF TEST SUMMARY =====
6 tests: 5 passed, 1 failed, 0 skipped

Failed:
    l2_announcement_arp_no_entry: test failed
```

The Rust test fails if any eBPF test failed.

## The examples in this repository

`examples/` shows the framework as a user would use it:

- `examples/ebpf-programs`: production programs;
- `examples/ebpf-test-programs`: tests for them, including scapy-based packet tests;
- `examples/ebpf-tests`: the userspace test that runs them.

With [`just`](https://github.com/casey/just) and `jq` installed:

```sh
just build-ebpf   # build the example eBPF objects with the xtask
just test         # run the regular tests
just ebpf-test    # build, then run the eBPF tests as root (asks for your sudo password)
```

## Limitations and future work

### Limitations

- **Work in progress**: APIs, generated code and the wire format will change.
- **TC only in the runner**: the macros accept `tc` and `xdp`, but the runner currently loads and
  runs `tc` (`SchedClassifier`) programs only.
- **One Rust test**: all eBPF tests run inside a single `#[test]`, so `cargo test` filters do not
  select individual eBPF tests.
- **Naming-based discovery**: tests are found by the names the macros generate; a program that
  does not follow that scheme is silently not run.
- **Limited messages**: log arguments are integers only, and the results of one test must fit in
  an 8 KiB buffer.
- **Repository layout**: the path to the scapy script and its Python environment is currently
  resolved relative to this repository.
- **Privileges**: running the tests requires root or the equivalent capabilities.

### Future work

- **Packet builder**: build packets in an `arrange` program layer by layer (Ethernet, VLAN, IPv4,
  IPv6 and its extension headers, ARP, TCP, UDP, ICMP, tunnels such as VXLAN and GENEVE, ...)
  instead of from prepared bytes. The groundwork is in `gladia-ebpf/src/util/pktbld.rs`.
- **Packet verifier**: build the expected packet in an `assert` program the same way, compare it
  with the actual result, and show where they differ, field by field. This brings what scapy
  provides now into the framework itself, without the Python dependency.
- **XDP** support in the runner.
- **Configurable paths** for the scapy script and its environment, independent of this
  repository.

## License

Unless otherwise noted, this project is dual licensed under either the MIT License or
the Apache License, Version 2.0, at your option.

Some files derived from third-party projects remain under their original license
terms, as indicated by their file headers.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this project shall be dual licensed under the
MIT License and Apache License, Version 2.0, without any additional terms
or conditions.

## Acknowledgments

This project is an independent educational project and is not affiliated with or
endorsed by the Cilium or Aya projects. Small portions of the repository are
derived from upstream projects and retain their original copyright notices and
license headers.

Special thanks to the Cilium community for building and openly sharing a
production-grade eBPF networking platform that serves as an invaluable learning
resource.

- [Cilium](https://github.com/cilium/cilium) — the primary reference and
  inspiration for this project's design
- [Aya](https://github.com/aya-rs/aya) — the Rust eBPF library this project
  is built on
