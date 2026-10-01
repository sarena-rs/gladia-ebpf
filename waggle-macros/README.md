# waggle-macros

Internal crate of [waggle](https://crates.io/crates/waggle): the procedural
macros (`#[arrange]`, `#[act]`, `#[assert]`) behind
[waggle-ebpf](https://crates.io/crates/waggle-ebpf).

**Do not depend on this crate directly.** The macros are re-exported by
`waggle-ebpf`, and the code they generate only compiles through it. This crate
has no stable API, and its version always matches `waggle-ebpf` exactly.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
