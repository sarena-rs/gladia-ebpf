# Design

This document describes how gladia works internally: how tests are written and discovered, how
they call into the production programs, how results travel back to userspace, and why it is built
the way it is. For usage, see the [README](../README.md); for the background, the blog post
[Testing eBPF code where it actually runs](https://erwinkok.org/posts/ebpf-testing-framework/).

> gladia is work in progress. This document describes the current state, including its rough
> edges; see [Limitations and future work](#limitations-and-future-work).

## Goals

- **Test the real thing.** The code under test is the compiled, verifier-checked production
  object, run in the kernel, not a userspace build or a stand-in.
- **Tests are eBPF programs.** Setup and checks run in kernel space too, where the real execution
  context (the `__sk_buff`, packet data, maps) is available.
- **No changes to production code.** Tests reach production programs by name; the production
  crate does not know it is being tested.
- **No build strategy imposed.** How eBPF objects are compiled (aya-build, an xtask, ...) and with
  which toolchain or flags is up to the user.

## Overview

```text
             build time                                   test time (userspace runner)
 ┌─────────────────────────────┐
 │ my-programs (production)    │ ── cargo build ──► my-programs.o ──────────┐
 └─────────────────────────────┘                                            │ load
 ┌─────────────────────────────┐                                            ▼
 │ my-test-programs            │                                  ┌───────────────────┐
 │  build.rs                   │                                  │ gladia runner     │
 │   gladia::build_mapping()   │                                  │  1. load objects  │
 │    scan src/ for tail_call! │                                  │  2. fill tail     │
 │    generate map, table,     │ ── cargo build ──► my-test-      │     call map      │
 │    and tail_call! macro     │                    programs.o ──►│  3. find tests    │
 │  #[arrange] #[act] #[assert]│                                  │  4. run each test │
 └─────────────────────────────┘                                  │     in the kernel │
                                                                  │  5. report        │
                                                                  └───────────────────┘
```

The framework is split into four crates:

- **`gladia-ebpf`** (`no_std`): what test authors use on the eBPF side. It re-exports the
  macros, and provides the `TestSuite` that collects results, the assertion and logging macros,
  the result maps, and helpers such as `PacketBuilder`.
- **`gladia-macros`**: the procedural macros `#[arrange]`, `#[act]`, `#[assert]` and
  `include_generated!`.
- **`gladia`**: the userspace side. It contains the runner, and `build_mapping()`, the code
  generator that runs in the build script of the test crate.
- **`gladia-shared`**: types and the wire format used by both sides.

## Writing tests

### Arrange, act, assert

A test is a group of up to three eBPF programs that share a test name:

| Program | Signature | Role |
|---|---|---|
| `#[arrange(tc, "name")]` | `fn(TcContext) -> TestStatus` | optional; prepares the input, typically the packet |
| `#[act(tc, "name")]` | `fn(TcContext) -> TestStatus` | optional; runs the code under test, typically with `tail_call!` |
| `#[assert(tc, "name")]` | `fn(TcContext, &mut TestSuite)` | required; checks the outcome and records the result |

The runner executes them in that order with `BPF_PROG_TEST_RUN`, feeding the output packet and
context of one program into the next.

### What the macros generate

eBPF programs are found by the loader through their symbols, and the runner finds tests by name
(see [Discovering tests](#discovering-tests)). The macros therefore give every program a
predictable name:

```text
__test_fw_<kind>_<test name>        e.g. __test_fw_assert_arp_request_is_answered
```

The user's function is kept as is, and a wrapper with that name is generated around it. For `tc`,
the wrapper is a `#[no_mangle]` function in the `classifier` section that converts the raw
`*mut __sk_buff` into a `TcContext` and calls the user's function:

```rust
#[unsafe(no_mangle)]
#[unsafe(link_section = "classifier")]
pub fn __test_fw_arrange_arp_request_is_answered(ctx: *mut __sk_buff) -> i32 {
    let ctx = unsafe { NonNull::new_unchecked(ctx) };
    arp_request_arrange(TcContext::new(ctx)) as i32
}
```

The `assert` wrapper does more. It creates a `TestSuite`, which starts the result record for the
test, passes it to the user's function, and finally writes the test status into the record and
returns it. The body of the user's function is wrapped in a `loop { ...; break; }`: that lets
`test_fatal!`, `test_skip!` and a failing `assert_test!` end the test early with a plain `break`,
without needing a `return` value or unwinding, neither of which fits eBPF.

The generated code refers to everything through `::gladia_ebpf::...` paths (including Aya, via a
hidden `__private` re-export), so the test crate only needs to depend on `gladia-ebpf`.

## Reporting results

eBPF programs have no allocator and no formatting machinery, so results are written as a compact
binary record into a fixed buffer, and turned into text in userspace.

### The result record

`gladia-ebpf` defines the result maps:

| Map | Contents |
|---|---|
| `test_suite_result` | one 8 KiB byte array: the result record of the current test |
| `test_suite_status_code` | the return value of the `act` program |
| `scapy_assert_map`, `scapy_assert_map_count` | failed buffer comparisons, for packet diffs |

The record is a sequence of tag-length-value (TLV) entries written by `TlvWriter`: a version, the
test name and source file, any number of log entries, and finally the status. Tags are defined in
`gladia-shared/src/wire.rs`.

A log entry consists of the source line, the format string, and its arguments as raw `u64`
values. The format string is a literal, so it is stored in the object file and only copied into
the record. `test_log!` and friends pick an implementation by argument count (`log0` to `log3`,
then a slice), keeping the common cases cheap for the verifier.

### Status

A test ends in one of four states: `Pass`, `Fail`, `Skip` or `FrameworkError`. The last one marks
problems in the framework itself, such as a full result buffer or a failed tail call, as opposed
to a failing assertion.

### In userspace

After the `assert` program has run, the runner reads `test_suite_result`, trims the trailing
zeroes (the record has no explicit length), and parses it with `tlv_reader`. `report::format_log`
substitutes the arguments into the format strings, supporting the specifiers of
`bpf_trace_printk`. The runner prints the verdict and messages per test, and a summary at the end.

## Calling production programs

The `act` program has to run the actual production code. It does so with a **tail call** into the
production program, through a `ProgramArray` map in the test object that the runner fills with
the production programs.

The difficulty is the bookkeeping. A tail call takes an index into the program array, but test
authors want to write a program name. Something has to know which programs the tests call, give
each a slot, size the map, and tell the runner which program belongs in which slot, all without
the user maintaining a list. gladia solves this with code generation in the test crate's build
script.

### Finding the calls

`gladia::build_mapping()` runs in the build script of the test crate. It walks the crate's `src/`
directory, parses every `.rs` file with `syn`, and visits all macro invocations. For every
invocation of `tail_call!`, it takes the second argument, which must be a string literal: the
name of the production program.

```rust
tail_call!(&ctx, "from_netdev")   // collects "from_netdev"
```

Anything else fails the build with a message that names the file and the call: a wrong number of
arguments, or a name that is not a string literal. The names must be literals because they have to be known
at build time.

The build script emits `cargo:rerun-if-changed` for the source directory, so cargo reruns it
whenever a source file is added, removed or changed. A call that is used in the source therefore
always has generated code by the time the source is compiled.

### Assigning indices

The calls are collected into a sorted set. Each unique name gets the index of its position in that
set, so the assignment is stable across builds and independent of the order in which files are
visited. A call that is used in several places gets one slot.

### The generated code

The build script writes `$OUT_DIR/__gladia_tail_calls.rs`, which contains three items. For a crate
that calls `from_netdev` and `to_host`, it looks like this:

```rust
// 1. The program array, sized to the number of unique calls.
#[aya_ebpf::macros::map(name = "__gladia_tail_call_map")]
pub static __gladia_tail_call_map: ProgramArray = ProgramArray::with_max_entries(2, 0);

// 2. A table mapping each slot to its program name, for the runner.
#[used]
#[unsafe(link_section = ".gladia_tail_call_section")]
pub static __gladia_TAIL_CALL_MAP: TestEntryHeader<2> = TestEntryHeader {
    version: 1,
    count: 2,
    size: size_of::<TestEntryCall>() as u32,
    entries: [
        TestEntryCall { index: 0, name: *b"from_netdev\0\0..." },
        TestEntryCall { index: 1, name: *b"to_host\0\0\0\0..." },
    ],
};

// 3. The macro the tests use: one arm per call, plus a catch-all.
#[macro_export]
macro_rules! tail_call {
    ($ctx:expr, "from_netdev") => {{
        let _ = unsafe { $crate::__gladia_tail_call_map.tail_call($ctx, 0u32) };
        gladia_ebpf::TestStatus::FrameworkError
    }};
    ($ctx:expr, "to_host") => {{ /* same, with index 1 */ }};
    ($ctx:expr, $name:literal) => {{
        compile_error!(concat!("unknown tail call `", $name, "`; rebuild so the build script picks it up"));
        gladia_ebpf::TestStatus::FrameworkError
    }};
}
```

Some details:

- **The macro is a value.** A successful tail call never returns, so the block only evaluates to
  `TestStatus::FrameworkError` when the call fails, for example because the slot is empty. That
  lets an `act` program simply end with `tail_call!(&ctx, "from_netdev")`.
- **The names are matched as literals.** `macro_rules!` matches the string literal token, so a
  call compiles to a constant index; there is no lookup at run time.
- **The catch-all arm** never matches under cargo, because every name in the source has its own
  arm. It is there for tools that may see stale generated code, such as an editor that has not
  rerun the build script yet: they get a clear message instead of a confusing type error. For the
  same reason, the macro is generated even when there are no calls.
- **`#[macro_export]` and `$crate`** make the macro usable from a binary target that depends on
  the library that includes the generated file.
- **Names are limited** to 64 bytes (`STRING_SIZE`), the size of the name field in the table; a
  longer name fails the build.
- **Without calls**, no map or table is generated: a program array with zero entries would be
  rejected by the kernel.

### Including the generated code

`gladia_ebpf::include_generated!()` expands to an `include!` of the generated file. Because
`macro_rules!` macros are only visible after their definition, it must come at the top of the
crate root, before the `mod` declarations. The generated items must exist only once per object,
so it is included in one crate only, typically the library of the test crate.

### The table

The table is how the build-time knowledge reaches the runner, through the object file itself. It
lives in its own ELF section, `.gladia_tail_call_section`, and `#[used]` keeps the linker from
removing it. Its layout is `#[repr(C)]` and defined in `gladia-shared`:

```text
TestEntryHeader                         TestEntryCall (repeated `count` times)
┌──────────┬──────────┬──────────┐      ┌──────────┬──────────────────────────┐
│ version  │ size     │ count    │      │ index    │ name                     │
│ u32 (1)  │ u32      │ u32      │      │ u32      │ [u8; 64], zero padded    │
└──────────┴──────────┴──────────┘      └──────────┴──────────────────────────┘
```

`size` is the size of one entry. The runner checks it, together with `version`, before reading the
entries, so a mismatch between the framework versions on both sides is reported instead of
misread.

There is a single table for the whole crate. An earlier version generated one table per source
file, but that only duplicated names that are used in several files, without giving the runner
anything it needs.

### Wiring at test time

Before running any test, the runner:

1. reads the table from `.gladia_tail_call_section` in the test object (with the `object` crate,
   without the kernel);
2. loads every listed program from the production object, by name;
3. puts each program's file descriptor in its slot of `__gladia_tail_call_map`.

Only the production programs that the tests actually call are loaded. The production object
stays loaded for the whole run, because the program array refers to its programs.

## The runner

`gladia::run_ebpf_test(programs, test_programs)` takes the two objects as bytes and:

1. **Prepares the pin directory** (`/sys/fs/bpf/gladia` by default), emptying it first, with
   separate subdirectories for the production and test maps.
2. **Loads both objects** with Aya.
3. **Wires the tail calls** as described above.
4. **Discovers the tests** by grouping the test object's programs on the
   `__test_fw_<kind>_<name>` scheme. Every test must have an `assert` program, and at most one
   program of each kind; anything else is an error before any test runs.
5. **Loads all test programs.**
6. **Runs every test**, one after the other:
   - it clears the result maps, so no state leaks from the previous test;
   - it runs `arrange`, `act` and `assert` with `BPF_PROG_TEST_RUN`, each on the packet and
     context produced by the previous one, starting from a zeroed packet;
   - it stores the return value of `act` in `test_suite_status_code`, where the assert program can
     check it;
   - it reads, parses and prints the result record;
   - it hands any failed buffer comparisons to the packet diff (see below).

   A failing test does not stop the run: every test runs, and failures are collected.
7. **Prints a summary** and returns an error if any test failed, which makes the surrounding Rust
   test fail.

An `arrange` or `act` program returning `Fail` or `FrameworkError` fails the test. Errors that
prevent any test from running, such as a program that fails to load, stop the run.

## Packet assertions with scapy

Comparing packets byte by byte gives poor failure messages. `assert_buffer!` compares a region of
the packet with an expected buffer, and on a mismatch records both, together with the scapy layer
to decode them from (for example `"Ether"`), in `scapy_assert_map`.

After the test, the runner serializes the recorded comparisons to JSON and pipes them into
`scapy/trace_diff_pkts.py` (derived from Cilium), which decodes both packets with scapy and prints
a field-by-field diff. The script runs with the Python environment in `scapyenv/`.

This depends on Python and on the layout of this repository, and the expected packet has to be
prepared as bytes up front. The planned packet builder and verifier (see
[Limitations and future work](#limitations-and-future-work)) are meant to replace it.

## Building the eBPF objects

gladia does not build eBPF objects. The runner takes them as byte slices, and the user decides how
to produce them:

- with **aya-build**, from the build script of the userspace test crate, embedding the objects
  from `OUT_DIR`;
- with **a separate build step**, such as the xtask in this repository, embedding the objects from
  a known directory.

Both embed the bytes with `aya::include_bytes_aligned!`. The only build-time requirement gladia
places on the eBPF side is `gladia::build_mapping()` in the build script of the test crate, which
works with any strategy, because each of them builds that crate with cargo.

Building the objects from within the framework was considered and rejected. A nested cargo build
in a build script needs its own target directory to avoid deadlocking on cargo's lock, either
floods the output with forwarded messages or stays silent for a long time, and adds rebuild
tracking that has to know every input. It would also force one toolchain and set of flags on all
users.

## Design decisions

- **A build script, not a procedural macro, finds the calls.** A procedural macro only sees the
  tokens it is applied to; it cannot know all tail calls in a crate to size a map or assign
  indices. A build script can read the whole source tree.
- **Literal names.** Requiring string literals keeps the call set statically known, lets the macro
  resolve a name to a constant index at compile time, and turns typos into build errors instead
  of failed tail calls at run time.
- **Sorted, unique indices.** Stable indices make the generated code reproducible and the output
  of the build script deterministic, regardless of file system order.
- **The object file carries the mapping.** Embedding the table in its own section means the runner
  needs nothing but the object: no side files, environment variables or generated userspace code
  that could get out of sync.
- **Tests run sequentially.** All tests share the result maps, so they run one at a time, and the
  maps are cleared before each test.

## Limitations and future work

### Limitations

- **TC only.** The macros accept `tc` and `xdp`, but the runner loads and runs `SchedClassifier`
  programs only. XDP support in the runner is still to be done.
- **One Rust test.** All eBPF tests run inside a single `#[test]`; the runner's own output reports
  them individually, but `cargo test` filters cannot select them.
- **Naming-based discovery.** A program that does not follow the `__test_fw_` scheme is silently
  not run. In practice the macros always generate correct names.
- **Integer-only log arguments**, and an 8 KiB result buffer per test.
- **Literal-only tail call names**, of at most 64 bytes.
- **Repository-relative scapy paths.** The runner finds `scapy/trace_diff_pkts.py` and `scapyenv/`
  relative to the `gladia` crate in this repository; this needs to become configurable before the
  crates are published.
- **Root required.** Loading programs and `BPF_PROG_TEST_RUN` require root or the equivalent
  capabilities.

### Future work

- **Packet builder.** `gladia-ebpf/src/util/pktbld.rs` is the start of a packet builder for
  `arrange` programs. It already tracks the layers of a packet (`PktLayer`, with an offset per
  layer, up to seven layers: three outer headers, a tunnel header and three inner headers), but so
  far only copies prepared bytes into the packet. The goal is to build packets layer by layer
  (Ethernet, 802.1Q, IPv4, IPv6 with its extension headers, ARP, TCP, UDP, ICMP, ICMPv6, SCTP,
  VXLAN, GENEVE, ...), filling in lengths and checksums, so tests describe their input instead of
  embedding byte arrays generated elsewhere.
- **Packet verifier.** The counterpart for `assert` programs: build the expected packet with the
  same builder, compare it with the packet the program under test produced, and report the
  differences per layer and field. This brings what the scapy integration offers into the
  framework itself, in Rust, without Python and without preparing expected packets up front. The
  differences could travel back to userspace in the result record, like log entries.
- **XDP in the runner**, next to TC.
- **Configurable scapy paths**, as long as the scapy integration remains.
