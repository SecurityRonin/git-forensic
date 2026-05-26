use crate::error::Result;
use crate::hash::GitHash;

#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub mode: u32,
    pub name: String,
    pub hash: GitHash,
}

impl TreeEntry {
    pub fn is_directory(&self) -> bool { todo!() }
    pub fn is_executable(&self) -> bool { todo!() }
    pub fn is_symlink(&self) -> bool { todo!() }
}

#[derive(Debug, Clone)]
pub struct TreeObject {
    pub hash: GitHash,
    pub entries: Vec<TreeEntry>,
}

impl TreeObject {
    pub fn parse(hash: GitHash, data: &[u8]) -> Result<Self> {
        todo!()
    }
}
