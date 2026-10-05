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

#[derive(Debug, thiserror::Error)]
pub enum TestRunnerError {
    #[error("eBPF program error: {0}")]
    EbpfError(#[from] EbpfError),

    #[error("eBPF program error: {0}")]
    ProgramError(#[from] ProgramError),

    #[error("Program {0} not found")]
    ProgramNotFound(String),

    #[error("eBPF map error: {0}")]
    MapError(#[from] MapError),

    #[error("Map {0} not found")]
    MapNotFound(String),

    #[error("Object file error: {0}")]
    ObjectError(#[from] object::Error),

    #[error("Unsupported tail call table version {0}, expected 1")]
    UnsupportedVersion(u32),

    #[error("Tail call table entry size is {size}, expected {expected}")]
    EntrySizeMismatch { size: usize, expected: usize },

    #[error("Test '{0}' has no assert program")]
    MissingCheck(String),

    #[error("Test has no result")]
    NoResult,

    #[error("Multiple {kind} programs found for test '{test}'")]
    DuplicateProgram { kind: String, test: String },

    #[error("{0}")]
    TestFailed(String),

    #[error("Error while tracing diff packets: exited with {0}")]
    TraceDiff(std::process::ExitStatus),

    #[error("{failed} of {total} eBPF tests failed")]
    TestsFailed { failed: usize, total: usize },

    #[error("Parse failed: {0}")]
    ParseError(#[from] ParseError),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] Utf8Error),

    #[error("JSON serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Res<T> = Result<T, TestRunnerError>;
