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
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct RawObject {
    pub kind: ObjectKind,
    pub data: Vec<u8>,
    pub verified: bool,
}
