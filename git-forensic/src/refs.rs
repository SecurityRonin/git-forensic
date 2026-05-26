use std::path::Path;
use crate::error::Result;
use crate::hash::GitHash;

pub fn resolve_ref(git_dir: &Path, refname: &str) -> Result<GitHash> {
    todo!()
}
