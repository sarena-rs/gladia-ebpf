//! A test framework for eBPF programs written with [Aya](https://aya-rs.dev): the userspace
//! runner.
//!
//! Tests are eBPF programs themselves, written with
//! [gladia-ebpf](https://docs.rs/gladia-ebpf) in arrange/act/assert style. This crate loads the
//! production programs and their tests, runs each test in the kernel with `BPF_PROG_TEST_RUN`,
//! and reports the results, including assertion failures and log messages from the eBPF side.
//! Because the tests run as BPF bytecode, they test the exact code that is deployed.
//!
//! This crate has two roles:
//!
//! - as a **build dependency** of the eBPF test crate, its [`build_mapping`] generates the code
//!   that lets tests tail call into production programs by name;
//! - as a **dev-dependency** of a userspace crate, [`run_ebpf_test`] runs the tests.
//!
//! # Running the tests
//!
//! The runner takes the two compiled eBPF objects as bytes: the production programs and the test
//! programs. How they are built is up to you, for example with `aya-build` in a build script, or
//! with a separate build step. Embed them with `aya::include_bytes_aligned!`, so they can be
//! parsed in place:
//!
//! ```rust,ignore
//! // my-tests/tests/ebpf.rs
//! static PROGRAMS: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-programs"));
//! static TEST_PROGRAMS: &[u8] =
//!     aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/my-test-programs"));
//!
//! #[test]
//! #[ignore = "requires root"]
//! fn ebpf() -> gladia::Res<()> {
//!     gladia::run_ebpf_test(PROGRAMS, TEST_PROGRAMS)
//! }
//! ```
//!
//! Loading eBPF programs requires root, or `CAP_BPF` and `CAP_NET_ADMIN`, so the test is
//! `#[ignore]`d and run explicitly, for example with `sudo <test binary> --ignored --nocapture`.
//! Each eBPF test prints its verdict with its log messages, and the run ends with a summary. The
//! Rust test fails if any eBPF test failed.
//!
//! The runner currently runs `tc` (`SchedClassifier`) programs only.

#![warn(missing_docs)]

use std::str::Utf8Error;

use aya::{EbpfError, maps::MapError, programs::ProgramError};
use gladia_shared::tlv_reader::ParseError;

mod builder;
mod collect;
mod constants;
mod reader;
mod report;
mod test_runner;

pub use builder::build_mapping;
pub use test_runner::{DEFAULT_PIN_DIR, run_ebpf_test, run_ebpf_test_with_pin_dir};

/// An error of the test runner, or failed tests.
#[derive(Debug, thiserror::Error)]
pub enum TestRunnerError {
    /// Loading an eBPF object failed.
    #[error("eBPF program error: {0}")]
    EbpfError(#[from] EbpfError),

    /// Loading or running a program failed.
    #[error("eBPF program error: {0}")]
    ProgramError(#[from] ProgramError),

    /// A program is not in its object.
    #[error("Program {0} not found")]
    ProgramNotFound(String),

    /// Accessing a map failed.
    #[error("eBPF map error: {0}")]
    MapError(#[from] MapError),

    /// A map is not in its object.
    #[error("Map {0} not found")]
    MapNotFound(String),

    /// The test object could not be parsed.
    #[error("Object file error: {0}")]
    ObjectError(#[from] object::Error),

    /// The tail call table in the test object has an unknown version.
    #[error("Unsupported tail call table version {0}, expected 1")]
    UnsupportedVersion(u32),

    /// The entries of the tail call table in the test object have an unexpected size.
    #[error("Tail call table entry size is {size}, expected {expected}")]
    EntrySizeMismatch {
        /// The entry size in the object.
        size: usize,
        /// The entry size this version expects.
        expected: usize,
    },

    /// A test has no `assert` program.
    #[error("Test '{0}' has no assert program")]
    MissingCheck(String),

    /// The `assert` program of a test wrote no result.
    #[error("Test has no result")]
    NoResult,

    /// A test has more than one program of a kind.
    #[error("Multiple {kind} programs found for test '{test}'")]
    DuplicateProgram {
        /// The kind: `arrange`, `act` or `assert`.
        kind: String,
        /// The name of the test.
        test: String,
    },

    /// The `arrange` or `act` program of a test failed.
    #[error("{0}")]
    TestFailed(String),

    /// The script that shows the packet diff of a failed buffer assertion failed.
    #[error("Error while tracing diff packets: exited with {0}")]
    TraceDiff(std::process::ExitStatus),

    /// Some of the eBPF tests failed.
    #[error("{failed} of {total} eBPF tests failed")]
    TestsFailed {
        /// The number of failed tests.
        failed: usize,
        /// The number of tests.
        total: usize,
    },

    /// The result of a test could not be decoded.
    #[error("Parse failed: {0}")]
    ParseError(#[from] ParseError),

    /// A string from the test object is not valid UTF-8.
    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] Utf8Error),

    /// Serializing a failed buffer assertion failed.
    #[error("JSON serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    /// An I/O operation failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// The result type of the test runner.
pub type Res<T> = Result<T, TestRunnerError>;
