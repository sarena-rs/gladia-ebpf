export PATH := justfile_directory() / "scapyenv/bin" + ":" + env_var("PATH")

default:
  @just --list

setup:
    python3 -m venv scapyenv
    scapyenv/bin/pip install -r scapy/requirements.txt

# Build all workspace packages; the eBPF programs are built first
build: build-ebpf
    cargo build

# Quick compilation check without producing binaries
check: build-ebpf
    cargo check

# clean up target
clean:
    cargo clean    
    rm -rf target-ebpf    

# fmt up target
fmt:
    cargo fmt

# clippy
clippy: build-ebpf
    cargo clippy

# Run all tests except the eBPF test runner (requires root)
test: build-ebpf
    cargo test --workspace \
        --features test-util \
        --exclude sarena-test-runner \
        --exclude sarena-ebpf-programs \
        --exclude sarena-ebpf-test-programs \
        -- --no-capture

# Build eBPF programs (outputs to ./target-ebpf/)
build-ebpf:
    cargo xtask build-ebpf

ebpf-test: build-ebpf 
    #!/usr/bin/env bash
    set -euo pipefail
    exe=$(cargo test --no-run -p sarena-test-runner --message-format=json \
        | jq -r 'select(.profile.test == true) | .executable')
    sudo "$exe" --ignored --no-capture

# Full workflow: build, test, and run all root-only test suites incl. the eBPF tests
all: build test ebpf-test

# Run all `#[ignore]`d integration tests (requiring root/CAP_NET_ADMIN) for `package`
_root-test package: build-ebpf
    #!/usr/bin/env bash
    set -euo pipefail
    exes=$(cargo test -p {{package}} --features test-util --tests --no-run --message-format=json \
        | jq -r 'select(.profile.test == true) | .executable | select(. != null)')
    for exe in $exes; do
        just netns-clean
        sudo "$exe" --ignored --no-capture
    done
