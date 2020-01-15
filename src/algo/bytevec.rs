use std::fmt::Debug;
use std::{fmt, ops};

/// Wrapper around `Vec<u8>` for efficient `AlgoIo` conversions when working with byte data
///
/// Serde JSON serializes/deserializes `Vec<u8>` as an array of numbers.
/// This type provides a more direct way of converting to/from `AlgoIo` without going through JSON.
#[derive(Clone, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ByteVec {
    bytes: Vec<u8>,
}

impl ByteVec {
    /// Construct a new, empty `ByteVec`.
    pub fn new() -> Self {
        ByteVec::from(Vec::new())
    }

    /// Construct a new, empty `ByteVec` with the specified capacity.
    pub fn with_capacity(cap: usize) -> Self {
        ByteVec::from(Vec::with_capacity(cap))
    }

    /// Wrap existing bytes in a `ByteVec`.
    pub fn from<T: Into<Vec<u8>>>(bytes: T) -> Self {
        ByteVec {
            bytes: bytes.into(),
        }
    }
}

impl Debug for ByteVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.bytes, f)
    }
}

impl From<ByteVec> for Vec<u8> {
    fn from(wrapper: ByteVec) -> Vec<u8> {
        wrapper.bytes
    }
}

impl From<Vec<u8>> for ByteVec {
    fn from(bytes: Vec<u8>) -> Self {
        ByteVec::from(bytes)
    }
}

impl AsRef<Vec<u8>> for ByteVec {
    fn as_ref(&self) -> &Vec<u8> {
        &self.bytes
    }
}

impl AsRef<[u8]> for ByteVec {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl AsMut<Vec<u8>> for ByteVec {
    fn as_mut(&mut self) -> &mut Vec<u8> {
        &mut self.bytes
    }
}

impl AsMut<[u8]> for ByteVec {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

impl ops::Deref for ByteVec {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.bytes[..]
    }
}

impl ops::DerefMut for ByteVec {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.bytes[..]
    }
}
