use crate::error::Result;
use crate::hash::GitHash;

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub email: String,
    pub timestamp: i64,
    pub tz_offset_secs: i32,
}

#[derive(Debug, Clone)]
pub struct CommitObject {
    pub hash: GitHash,
    pub tree: GitHash,
    pub parents: Vec<GitHash>,
    pub author: Signature,
    pub committer: Signature,
    pub message: String,
}

impl CommitObject {
    pub fn parse(hash: GitHash, data: &[u8]) -> Result<Self> {
        todo!()
    }
}
