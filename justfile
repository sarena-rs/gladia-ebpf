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

# fmt up target
fmt:
    cargo fmt

# clippy
clippy: build-ebpf
    cargo clippy

# Run all tests except the eBPF test runner (requires root)
test: build-ebpf
    cargo test --workspace \
        --exclude waggle \
        --exclude ebpf-programs \
        --exclude ebpf-test-programs \
        -- --no-capture

# Build eBPF programs
build-ebpf:
    cargo run --release --package xtask -- build-ebpf -p ebpf-programs --const PROGRAMS -o target/ebpf-objects/ebpf-programs.rs
    cargo run --release --package xtask -- build-ebpf -p ebpf-test-programs --const TEST_PROGRAMS -o target/ebpf-objects/ebpf-test-programs.rs

ebpf-test: build-ebpf 
    #!/usr/bin/env bash
    set -euo pipefail
    exe=$(cargo test --no-run -p waggle --message-format=json \
        | jq -r 'select(.profile.test == true) | .executable')
    sudo "$exe" --ignored --no-capture

# Full workflow: build, test, and run all root-only test suites incl. the eBPF tests
all: build test ebpf-test

# Run all `#[ignore]`d integration tests (requiring root/CAP_NET_ADMIN) for `package`
_root-test package: build-ebpf
    #!/usr/bin/env bash
    set -euo pipefail
    exes=$(cargo test -p {{package}} --tests --no-run --message-format=json \
        | jq -r 'select(.profile.test == true) | .executable | select(. != null)')
    for exe in $exes; do
        just netns-clean
        sudo "$exe" --ignored --no-capture
    done
