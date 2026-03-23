#![allow(dead_code)]

use byteorder::{BigEndian, WriteBytesExt};
use std::io::Write;

/// Protocol identifiers
const FILE_IDENTIFIER: [u8; 4] = [0x4E, 0x5A, 0x54, 0x44]; // NZTD
const FILENAME_IDENTIFIER: [u8; 4] = [0x4E, 0x5A, 0x46, 0x4E]; // NZFN
const ERROR_IDENTIFIER: [u8; 4] = [0x4E, 0x45, 0x52, 0x52]; // NERR
pub const COMPLETE_IDENTIFIER: [u8; 4] = [0x4E, 0x5A, 0x55, 0x50]; // NZUP

/// Create a directory listing header with path
pub fn create_dir_header(path: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + path.len());
    let _ = buf.write_all(&FILENAME_IDENTIFIER);
    let _ = buf.write_u32::<BigEndian>(path.len() as u32);
    let _ = buf.write_all(path.as_bytes());
    buf
}

/// Append a filename entry to a directory listing buffer
pub fn append_filename(buf: &mut Vec<u8>, name: &str, is_dir: bool) {
    buf.push(if is_dir { 1 } else { 0 });
    buf.push(name.len() as u8);
    buf.extend_from_slice(name.as_bytes());
}

/// Create a file download header with size
pub fn create_file_header(size: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(12);
    let _ = buf.write_all(&FILE_IDENTIFIER);
    let _ = buf.write_u64::<BigEndian>(size);
    buf
}

/// Create an error response
pub fn create_error(err: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + err.len());
    let _ = buf.write_all(&ERROR_IDENTIFIER);
    let _ = buf.write_all(err.as_bytes());
    buf
}
