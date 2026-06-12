use std::path::{Path, PathBuf};

use crate::commit::CommitObject;
use crate::error::{GitError, Result};
use crate::hash::GitHash;
use crate::loose;
use crate::object::{ObjectKind, RawObject};
use crate::pack;
use crate::refs;
use crate::tree::TreeObject;

pub struct GitRepo {
    /// The `.git` directory.
    git_dir: PathBuf,
    /// `.git/objects`
    objects_dir: PathBuf,
}

impl GitRepo {
    /// Open a git repository. `path` may be the work-tree root (contains `.git/`)
    /// or the bare `.git` directory itself.
    pub fn open(path: &Path) -> Result<Self> {
        let git_dir = if path.join("HEAD").exists() {
            path.to_owned()
        } else if path.join(".git").join("HEAD").exists() {
            path.join(".git")
        } else {
            return Err(GitError::InvalidObject(format!(
                "not a git repository: {}",
                path.display()
            )));
        };

        let objects_dir = git_dir.join("objects");
        Ok(Self {
            git_dir,
            objects_dir,
        })
    }

    /// Resolve HEAD to its commit hash.
    pub fn head(&self) -> Result<GitHash> {
        refs::resolve_ref(&self.git_dir, "HEAD")
    }

    /// Resolve any ref name (e.g. `"HEAD"`, `"refs/heads/main"`, or a bare hex hash).
    pub fn resolve_ref(&self, name: &str) -> Result<GitHash> {
        refs::resolve_ref(&self.git_dir, name)
    }

    /// Read and verify an object by hash, from a loose file or a packfile.
    ///
    /// Loose objects are tried first; if absent, every packfile is searched
    /// (resolving `OFS_DELTA`/`REF_DELTA` chains). A truly missing object yields
    /// [`GitError::ObjectNotFound`]; an unsupported pack *index* version yields
    /// the distinct [`GitError::PackfileUnsupported`] — never a misleading
    /// not-found.
    pub fn read_object(&self, hash: &GitHash) -> Result<RawObject> {
        match loose::read_loose(&self.objects_dir, hash) {
            Err(GitError::ObjectNotFound(h)) => match pack::read_packed(&self.objects_dir, hash)? {
                Some(obj) => Ok(obj),
                None => Err(GitError::ObjectNotFound(h)),
            },
            other => other,
        }
    }

    /// Read and parse a commit object.
    pub fn read_commit(&self, hash: &GitHash) -> Result<CommitObject> {
        let obj = self.read_object(hash)?;
        if obj.kind != ObjectKind::Commit {
            return Err(GitError::InvalidObject(format!(
                "{hash} is a {:?}, not a commit",
                obj.kind
            )));
        }
        CommitObject::parse(*hash, &obj.data)
    }

    /// Read and parse a tree object.
    pub fn read_tree(&self, hash: &GitHash) -> Result<TreeObject> {
        let obj = self.read_object(hash)?;
        if obj.kind != ObjectKind::Tree {
            return Err(GitError::InvalidObject(format!(
                "{hash} is a {:?}, not a tree",
                obj.kind
            )));
        }
        TreeObject::parse(*hash, &obj.data)
    }

    /// Read a blob object and return its raw bytes.
    pub fn read_blob(&self, hash: &GitHash) -> Result<Vec<u8>> {
        let obj = self.read_object(hash)?;
        if obj.kind != ObjectKind::Blob {
            return Err(GitError::InvalidObject(format!(
                "{hash} is a {:?}, not a blob",
                obj.kind
            )));
        }
        Ok(obj.data)
    }

    /// Walk the commit ancestry chain, newest-first (first-parent only).
    pub fn walk_commits(&self, from: GitHash) -> impl Iterator<Item = Result<CommitObject>> + '_ {
        CommitWalker {
            repo: self,
            next: Some(from),
        }
    }
}

struct CommitWalker<'a> {
    repo: &'a GitRepo,
    next: Option<GitHash>,
}

impl<'a> Iterator for CommitWalker<'a> {
    type Item = Result<CommitObject>;

    fn next(&mut self) -> Option<Self::Item> {
        let hash = self.next.take()?;
        match self.repo.read_commit(&hash) {
            Ok(commit) => {
                self.next = commit.parents.first().copied();
                Some(Ok(commit))
            }
            Err(e) => Some(Err(e)),
        }
    }
}
