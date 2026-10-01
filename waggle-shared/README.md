# waggle-shared

Internal crate of [waggle](https://crates.io/crates/waggle): the types and
wire format shared by the userspace runner and the eBPF side.

**Do not depend on this crate directly.** Use
[waggle](https://crates.io/crates/waggle) in userspace and
[waggle-ebpf](https://crates.io/crates/waggle-ebpf) in your eBPF programs. This
crate has no stable API, and its version always matches theirs exactly.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
