# gladia

A test framework for eBPF programs written with [Aya](https://aya-rs.dev).

Tests are eBPF programs written with
[gladia-ebpf](https://crates.io/crates/gladia-ebpf), in arrange/act/assert
style. This crate is the userspace runner. It loads your programs and their
tests, runs each test in the kernel with `BPF_PROG_TEST_RUN`, and reports the
results, including assertion failures and log messages from the eBPF side.

Add it as a dev-dependency and drive it from an integration test:

```toml
[dev-dependencies]
gladia = "0.1"
```

Running the tests loads eBPF programs into the kernel, which requires
`CAP_BPF`/`CAP_NET_ADMIN` (or root).

> **Status:** early development; the API will change.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
