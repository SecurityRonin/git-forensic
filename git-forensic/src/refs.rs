use std::path::Path;

use crate::error::{GitError, Result};
use crate::hash::GitHash;

/// Resolve a ref name to its target hash.
///
/// Handles:
/// - `HEAD` (may be symbolic or a detached commit hash)
/// - `refs/heads/<branch>`
/// - bare 40-hex strings
pub fn resolve_ref(git_dir: &Path, refname: &str) -> Result<GitHash> {
    if refname.len() == 40 && refname.chars().all(|c| c.is_ascii_hexdigit()) {
        return GitHash::from_hex(refname);
    }

    let ref_path = git_dir.join(refname);
    let content = std::fs::read_to_string(&ref_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            GitError::RefNotFound(refname.to_string())
        } else {
            GitError::Io(e)
        }
    })?;

    let content = content.trim();

    // Symbolic ref: "ref: refs/heads/main"
    if let Some(target) = content.strip_prefix("ref: ") {
        return resolve_ref(git_dir, target);
    }

    GitHash::from_hex(content)
        .map_err(|_| GitError::RefNotFound(format!("{refname}: invalid hash {:?}", content)))
}
