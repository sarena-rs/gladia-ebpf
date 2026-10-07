//! The tags and status codes of the encoded test result.

/// The version of the wire format, written as the first record of every test result.
pub const WIRE_VERSION: u8 = 1;

/// The tag of a record in an encoded test result.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    /// The wire format version, a `u8`.
    Version = 0x01,

    /// The name of the test.
    TestName = 0x10,
    /// The source file of the test.
    TestFile = 0x11,
    /// The final [`TestStatus`], a `u8`.
    TestStatus = 0x13,

    /// The format string of a log message; starts a new message.
    LogFmt = 0x20,
    /// An argument of the current log message, a `u64`.
    LogArg = 0x21,
    /// The source line of the current log message, a `u32`.
    LogLine = 0x22,
}

impl Tag {
    /// The tag with value `v`, if any.
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Version),

            0x10 => Some(Self::TestName),
            0x11 => Some(Self::TestFile),
            0x13 => Some(Self::TestStatus),

            0x20 => Some(Self::LogFmt),
            0x21 => Some(Self::LogArg),
            0x22 => Some(Self::LogLine),

            _ => None,
        }
    }
}

/// The outcome of a test, or of one of its programs.
#[must_use]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    /// The test passed.
    Pass = 101,
    /// The test failed.
    Fail = 102,
    /// The test was skipped.
    Skip = 103,
    /// The framework itself failed, for example because a tail call or a map access failed.
    FrameworkError = 105,
}

impl TestStatus {
    /// The status with value `v`; an unknown value is a [`TestStatus::FrameworkError`].
    pub const fn from_u8(v: u8) -> Self {
        match v {
            101 => Self::Pass,
            102 => Self::Fail,
            103 => Self::Skip,
            _ => Self::FrameworkError,
        }
    }
}
