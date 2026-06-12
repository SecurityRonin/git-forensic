use crate::error::{GitError, Result};
use crate::hash::GitHash;

#[derive(Debug, Clone)]
pub struct TreeEntry {
    /// POSIX file mode (e.g. 0o100644 for regular file, 0o40000 for dir).
    pub mode: u32,
    pub name: String,
    pub hash: GitHash,
}

impl TreeEntry {
    pub fn is_directory(&self) -> bool {
        self.mode == 0o40000
    }

    pub fn is_executable(&self) -> bool {
        self.mode == 0o100_755
    }

    pub fn is_symlink(&self) -> bool {
        self.mode == 0o120_000
    }
}

#[derive(Debug, Clone)]
pub struct TreeObject {
    pub hash: GitHash,
    pub entries: Vec<TreeEntry>,
}

impl TreeObject {
    /// Parse raw tree object bytes (after the object header is stripped).
    ///
    /// Git tree binary format: repeated records of
    /// `"<mode> <name>\0<20-byte SHA1>"`
    pub fn parse(hash: GitHash, data: &[u8]) -> Result<Self> {
        let mut entries = Vec::new();
        let mut pos = 0;

        while pos < data.len() {
            let nul = data[pos..]
                .iter()
                .position(|&b| b == 0)
                .ok_or_else(|| GitError::InvalidObject("tree entry missing NUL".into()))?;

            let header = std::str::from_utf8(&data[pos..pos + nul]).map_err(|e| {
                GitError::InvalidObject(format!("tree entry header not UTF-8: {e}"))
            })?;

            let (mode_str, name) = header
                .split_once(' ')
                .ok_or_else(|| GitError::InvalidObject("tree entry header missing space".into()))?;

            let mode = u32::from_str_radix(mode_str, 8)
                .map_err(|_| GitError::InvalidObject(format!("invalid mode: {mode_str:?}")))?;

            pos += nul + 1;

            if pos + 20 > data.len() {
                return Err(GitError::InvalidObject(
                    "tree entry truncated: no hash bytes".into(),
                ));
            }
            let entry_hash = GitHash::from_bytes(&data[pos..pos + 20])?;
            pos += 20;

            entries.push(TreeEntry {
                mode,
                name: name.to_string(),
                hash: entry_hash,
            });
        }

        Ok(Self { hash, entries })
    }
}
