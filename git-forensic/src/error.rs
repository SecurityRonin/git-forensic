use crate::hash::GitHash;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("object not found: {0}")]
    ObjectNotFound(GitHash),

    #[error(
        "object {0} not found among loose objects; the repository has packfile(s), \
         which are not yet supported — the object may be packed (this is not a \
         confirmed absence)"
    )]
    PackfileUnsupported(GitHash),

    #[error("invalid hash: {0}")]
    InvalidHash(String),

    #[error("invalid object: {0}")]
    InvalidObject(String),

    #[error("hash mismatch: expected {expected}, got {got}")]
    HashMismatch { expected: GitHash, got: GitHash },

    #[error("ref not found: {0}")]
    RefNotFound(String),

    #[error("deflate-bomb guard triggered: decompressed size exceeds limit")]
    DeflateBomb,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, GitError>;
