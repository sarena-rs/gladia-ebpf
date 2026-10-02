# gladia-macros

Internal crate of [gladia](https://crates.io/crates/gladia): the procedural
macros (`#[arrange]`, `#[act]`, `#[assert]`) behind
[gladia-ebpf](https://crates.io/crates/gladia-ebpf).

**Do not depend on this crate directly.** The macros are re-exported by
`gladia-ebpf`, and the code they generate only compiles through it. This crate
has no stable API, and its version always matches `gladia-ebpf` exactly.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. See [NOTICE.md](NOTICE.md) for
attributions.
