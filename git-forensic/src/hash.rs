use std::fmt;
use crate::error::{GitError, Result};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GitHash(pub [u8; 20]);

impl GitHash {
    pub fn from_hex(s: &str) -> Result<Self> {
        todo!()
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        todo!()
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        todo!()
    }

    pub fn object_path(&self) -> (String, String) {
        todo!()
    }
}

impl fmt::Debug for GitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GitHash(todo)")
    }
}

impl fmt::Display for GitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "todo")
    }
}
