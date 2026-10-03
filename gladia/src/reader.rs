use std::io::{self, Cursor, Read};

use gladia_shared::{STRING_SIZE, TestEntryCall};
use object::{Object as _, ObjectSection as _};

use crate::{Res, TestRunnerError};

/// Reads the `(index, name)` pairs of the tail calls the test programs make, as recorded by the
/// build script in the `.gladia_tail_call_section` section.
pub fn find_entry_calls(data: &[u8]) -> Res<Vec<(u32, String)>> {
    let mut entry_calls = Vec::new();
    let file = object::File::parse(data)?;
    for section in file.sections() {
        if section.name()? == ".gladia_tail_call_section" {
            let metadata = section.data()?;
            let little_endian = file.is_little_endian();
            let mut reader = Reader::new(metadata, little_endian);

            let version = reader.read_u32()?;
            if version != 1 {
                return Err(TestRunnerError::UnsupportedVersion(version));
            }

            let size = reader.read_u32()? as usize;
            let expected_size = core::mem::size_of::<TestEntryCall>();
            if size != expected_size {
                return Err(TestRunnerError::EntrySizeMismatch {
                    size,
                    expected: expected_size,
                });
            }

            let count = reader.read_u32()? as usize;
            for _ in 0..count {
                let index = reader.read_u32()?;
                let name = reader.read_str(STRING_SIZE)?;
                entry_calls.push((index, name.to_owned()));
            }
        }
    }
    Ok(entry_calls)
}

struct Reader<'a> {
    cursor: Cursor<&'a [u8]>,
    little_endian: bool,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], little_endian: bool) -> Self {
        Self {
            cursor: Cursor::new(data),
            little_endian,
        }
    }

    fn read_u32(&mut self) -> io::Result<u32> {
        let mut buf = [0u8; 4];
        self.cursor.read_exact(&mut buf)?;
        Ok(if self.little_endian {
            u32::from_le_bytes(buf)
        } else {
            u32::from_be_bytes(buf)
        })
    }

    fn read_str(&mut self, max_size: usize) -> io::Result<&'a str> {
        let start = self.cursor.position() as usize;
        let data = self.cursor.get_ref();

        let end = start
            .checked_add(max_size)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "size overflow"))?;

        if end > data.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "not enough data",
            ));
        }

        let bytes = &data[start..end];
        let len = bytes.iter().position(|&b| b == 0).unwrap_or(max_size);
        self.cursor.set_position(end as u64);

        std::str::from_utf8(&bytes[..len])
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8"))
    }
}
