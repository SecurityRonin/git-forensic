use crate::error::{GitError, Result};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GitHash(pub [u8; 20]);

impl GitHash {
    pub fn from_hex(s: &str) -> Result<Self> {
        if s.len() != 40 {
            return Err(GitError::InvalidHash(format!(
                "expected 40 hex chars, got {}: {:?}",
                s.len(),
                s
            )));
        }
        let mut bytes = [0u8; 20];
        hex::decode_to_slice(s, &mut bytes)
            .map_err(|e| GitError::InvalidHash(format!("{e}: {s:?}")))?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self> {
        b.try_into()
            .map(|arr: [u8; 20]| Self(arr))
            .map_err(|_| GitError::InvalidHash(format!("expected 20 bytes, got {}", b.len())))
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn object_path(&self) -> (String, String) {
        let hex = self.to_hex();
        (hex[..2].to_string(), hex[2..].to_string())
    }
}

impl fmt::Debug for GitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GitHash({})", self.to_hex())
    }
}

impl fmt::Display for GitHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}
