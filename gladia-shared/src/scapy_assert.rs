//! The record of a failed buffer assertion.

#[cfg(feature = "user")]
use serde::ser::{Serialize, SerializeStruct, Serializer};

/// The maximum number of bytes `assert_buffer!` compares.
pub const SCAPY_MAX_BUF: usize = 1518;
/// The maximum number of failed buffer assertions recorded per test.
pub const SCAPY_MAX_ASSERTS: usize = 256;
/// The size of the string fields of a [`ScapyAssert`], including the terminating zero.
pub const SCAPY_MAX_STR_LEN: usize = 128;

/// A failed `assert_buffer!` check: the expected and the actual bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ScapyAssert {
    /// The name of the assertion, zero terminated.
    pub name: [u8; SCAPY_MAX_STR_LEN],
    /// The source file of the assertion, zero terminated.
    pub file: [u8; SCAPY_MAX_STR_LEN],
    /// The source line of the assertion.
    pub line: u32,
    /// The protocol layer the buffers start with, such as `Ether`, zero terminated.
    pub first_layer: [u8; SCAPY_MAX_STR_LEN],
    /// The number of bytes compared.
    pub expected_len: usize,
    /// The expected bytes.
    pub expected_buf: [u8; SCAPY_MAX_BUF],
    /// The length of the packet from the compared offset.
    pub actual_len: usize,
    /// The bytes of the packet, or zeroes if the packet was too short.
    pub actual_buf: [u8; SCAPY_MAX_BUF],
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for ScapyAssert {}

#[cfg(feature = "user")]
impl Serialize for ScapyAssert {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ScapyAssert", 8)?;
        state.serialize_field("name", &convert::<S>(&self.name)?)?;
        state.serialize_field("file", &convert::<S>(&self.file)?)?;
        state.serialize_field("linenum", &self.line)?;
        state.serialize_field("first-layer", &convert::<S>(&self.first_layer)?)?;
        state.serialize_field("exp-len", &self.expected_len)?;
        state.serialize_field(
            "exp-buf",
            &encode_to_string(&self.expected_buf[..self.expected_len]),
        )?;
        state.serialize_field("got-len", &self.actual_len)?;
        state.serialize_field(
            "got-buf",
            &encode_to_string(&self.actual_buf[..self.actual_len]),
        )?;
        state.end()
    }
}

impl ScapyAssert {
    /// An all-zero assertion.
    pub const fn null() -> Self {
        Self {
            name: [0; SCAPY_MAX_STR_LEN],
            file: [0; SCAPY_MAX_STR_LEN],
            line: 0,
            first_layer: [0; SCAPY_MAX_STR_LEN],
            expected_len: 0,
            expected_buf: [0; SCAPY_MAX_BUF],
            actual_len: 0,
            actual_buf: [0; SCAPY_MAX_BUF],
        }
    }
}

/// An all-zero assertion, as a source of zeroes.
pub static SCAPY_ASSERT_NULL: ScapyAssert = ScapyAssert::null();

#[cfg(feature = "user")]
fn encode_to_string(bytes: &[u8]) -> String {
    let mut out = vec![0u8; bytes.len() * 2];
    hex::encode_to_slice(bytes, &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[cfg(feature = "user")]
fn convert<S>(bytes: &[u8; SCAPY_MAX_STR_LEN]) -> Result<String, S::Error>
where
    S: Serializer,
{
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    let x = str::from_utf8(&bytes[..len]).map_err(serde::ser::Error::custom)?;
    Ok(x.to_string())
}
