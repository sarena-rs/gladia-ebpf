# gladia-shared

Internal crate of [gladia](https://crates.io/crates/gladia): the types and
wire format shared by the userspace runner and the eBPF side.

**Do not depend on this crate directly.** Use
[gladia](https://crates.io/crates/gladia) in userspace and
[gladia-ebpf](https://crates.io/crates/gladia-ebpf) in your eBPF programs. This
crate has no stable API, and its version always matches theirs exactly.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
