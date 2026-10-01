use std::str::Utf8Error;

use aya::{EbpfError, maps::MapError, programs::ProgramError};
use waggle_shared::tlv_reader::ParseError;

mod ebpf_test_runner;
mod report;

pub use ebpf_test_runner::run_ebpf_test;

/// A compiled eBPF ELF object embedded in the binary.
#[derive(Debug, Clone, Copy)]
pub struct EbpfObject {
    /// Name of the eBPF package the object was built from.
    pub name: &'static str,
    /// The ELF bytes, aligned (via `aya::include_bytes_aligned!`) so they can
    /// be parsed in place.
    pub bytes: &'static [u8],
    /// Hex-encoded SHA-256 of `bytes`, for identifying the embedded build.
    pub sha256: &'static str,
}

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

    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Test '{0}' has no assert program")]
    MissingCheck(String),

    #[error("Test has no result")]
    NoResult,

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

//
//
//

pub fn build_mapping() {
    // let config = Config::new().scan_src().generate_mapping();
    // config.run().unwrap();

    // waggle::Builder::new()
    //     .scan("src")
    //     .generate("calls.rs")
    //     .run()
    //     .unwrap();
}

// Generate:
// #[doc(hidden)]
// pub mod __generated {
//   pub static CALLS: &[(&str, u32)] = &[
//     ("hello", 1"),
//     ("world", 2"),
// ];
// }

#[macro_export]
macro_rules! tail_call {
    ("hello") => {
        1u32
    };
    ("world") => {
        2u32
    };
}
