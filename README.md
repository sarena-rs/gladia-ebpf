# eBPF Test Framework

## Running the tests

The runner takes the two compiled eBPF objects as bytes: the production programs and the test
programs that exercise them.

```rust
gladia::run_ebpf_test(PROGRAMS, TEST_PROGRAMS)
```

How the objects are built is up to you; gladia does not impose a build strategy, toolchain or
compiler settings. Embed the objects with `aya::include_bytes_aligned!`, so they can be parsed in
place. The only requirement is that the crate with the test programs calls
`gladia::build_mapping()` from its `build.rs`.

### With aya-build

Build the eBPF packages from the `build.rs` of the crate that holds the test. The objects are placed in `OUT_DIR`:

```rust
// build.rs
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
// tests/ebpf.rs
static PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-programs"));
static TEST_PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-test-programs"));

#[test]
#[ignore = "requires root"]
fn ebpf() -> gladia::Res<()> {
    gladia::run_ebpf_test(PROGRAMS, TEST_PROGRAMS)
}
```

### With a separate build step

Build the objects beforehand (with an xtask, a script, ...) and point the test at them. This
repository does so for `examples/`: `just build-ebpf` writes the objects to `target/ebpf-objects`,
and `.cargo/config.toml` sets `EBPF_OBJECTS` to that directory:

```rust
static PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("EBPF_OBJECTS"), "/ebpf-programs.o"));
static TEST_PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("EBPF_OBJECTS"), "/ebpf-test-programs.o"));
```

Here, rebuilding the objects after changing the eBPF code is up to that build step:
`just ebpf-test` builds them before running the tests.


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

Theis project is an independent educational project and is not affiliated with or 
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
