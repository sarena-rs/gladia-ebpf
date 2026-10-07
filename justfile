default:
  @just --list

# Build all workspace packages; the eBPF programs are built first
build: build-ebpf
    cargo build

# Quick compilation check without producing binaries
check: build-ebpf
    cargo check

# clean up target
clean:
    cargo clean
    cargo clean --manifest-path examples/Cargo.toml

# fmt up target
fmt:
    cargo fmt
    cargo fmt --manifest-path examples/Cargo.toml

# clippy
clippy: build-ebpf
    cargo clippy

# Run all tests except the eBPF test runner (requires root)
test: build-ebpf
    cargo test --workspace \
        --exclude ebpf-tests \
        -- --no-capture

# Build eBPF programs
build-ebpf:
    cargo run --release --package xtask -- build-ebpf --manifest-path examples/Cargo.toml -p ebpf-programs -o target/ebpf-objects/ebpf-programs.o
    cargo run --release --package xtask -- build-ebpf --manifest-path examples/Cargo.toml -p ebpf-test-programs -o target/ebpf-objects/ebpf-test-programs.o

ebpf-test: (_root-test "ebpf-tests")

# Full workflow: build, test, and run all root-only test suites incl. the eBPF tests
all: build test ebpf-test

# Run all `#[ignore]`d integration tests (requiring root/CAP_NET_ADMIN) for `package`
_root-test package: build-ebpf
    #!/usr/bin/env bash
    set -euo pipefail
    exes=$(cargo test -p {{package}} --tests --no-run --message-format=json \
        | jq -r 'select(.profile.test == true) | .executable | select(. != null)')
    for exe in $exes; do
        sudo "$exe" --ignored --no-capture
    done
