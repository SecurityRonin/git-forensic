#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
pub mod commit;
pub mod error;
pub mod hash;
pub mod loose;
pub mod object;
pub mod pack;
pub mod reflog;
pub mod refs;
pub mod repo;
pub mod tree;

pub use commit::{CommitObject, Signature};
pub use error::{GitError, Result};
pub use hash::GitHash;
pub use object::{ObjectKind, RawObject};
pub use reflog::{parse_reflog, ReflogEntry};
pub use repo::GitRepo;
pub use tree::{TreeEntry, TreeObject};
