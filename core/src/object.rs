use crate::error::{GitError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Commit,
    Tree,
    Blob,
    Tag,
}

impl ObjectKind {
    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        match b {
            b"commit" => Ok(Self::Commit),
            b"tree"   => Ok(Self::Tree),
            b"blob"   => Ok(Self::Blob),
            b"tag"    => Ok(Self::Tag),
            other => Err(GitError::InvalidObject(format!(
                "unknown object kind: {:?}",
                String::from_utf8_lossy(other)
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawObject {
    pub kind: ObjectKind,
    pub data: Vec<u8>,
    /// True if the SHA1 of (header + data) matched the expected hash.
    pub verified: bool,
}
