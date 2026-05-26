use std::path::{Path, PathBuf};
use crate::commit::CommitObject;
use crate::error::Result;
use crate::hash::GitHash;
use crate::object::RawObject;
use crate::tree::TreeObject;

pub struct GitRepo {
    git_dir: PathBuf,
    objects_dir: PathBuf,
}

impl GitRepo {
    pub fn open(path: &Path) -> Result<Self> {
        todo!()
    }

    pub fn head(&self) -> Result<GitHash> {
        todo!()
    }

    pub fn resolve_ref(&self, name: &str) -> Result<GitHash> {
        todo!()
    }

    pub fn read_object(&self, hash: &GitHash) -> Result<RawObject> {
        todo!()
    }

    pub fn read_commit(&self, hash: &GitHash) -> Result<CommitObject> {
        todo!()
    }

    pub fn read_tree(&self, hash: &GitHash) -> Result<TreeObject> {
        todo!()
    }

    pub fn read_blob(&self, hash: &GitHash) -> Result<Vec<u8>> {
        todo!()
    }

    pub fn walk_commits(&self, from: GitHash) -> impl Iterator<Item = Result<CommitObject>> + '_ {
        std::iter::empty()
    }
}
