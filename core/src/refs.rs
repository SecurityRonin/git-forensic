use std::path::Path;

use crate::error::{GitError, Result};
use crate::hash::GitHash;

/// Enumerate every ref in `git_dir`: the loose `refs/**` tree, `packed-refs`,
/// and `HEAD` (when it resolves to a hash). Returns `(refname, target_hash)`
/// pairs. Unresolvable or malformed refs are skipped; never panics.
#[must_use]
pub fn list_refs(git_dir: &Path) -> Vec<(String, GitHash)> {
    let mut out: Vec<(String, GitHash)> = Vec::new();

    // Loose refs under refs/ (recursively).
    let refs_root = git_dir.join("refs");
    collect_loose_refs(&refs_root, "refs", git_dir, &mut out);

    // packed-refs: lines of "<sha> <refname>"; "^<sha>" peel lines are skipped
    // (the peeled tag commit is reachable via the tag object itself).
    if let Ok(text) = std::fs::read_to_string(git_dir.join("packed-refs")) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
                continue;
            }
            if let Some((sha, name)) = line.split_once(' ') {
                if out.iter().any(|(n, _)| n == name) {
                    continue; // a loose ref shadows the packed one.
                }
                if let Ok(hash) = GitHash::from_hex(sha.trim()) {
                    out.push((name.trim().to_string(), hash));
                }
            }
        }
    }

    // HEAD, resolved to a hash (symbolic or detached).
    if let Ok(hash) = resolve_ref(git_dir, "HEAD") {
        if !out.iter().any(|(n, _)| n == "HEAD") {
            out.push(("HEAD".to_string(), hash));
        }
    }

    out
}

/// Recursively walk a loose-ref directory, appending `(refname, hash)` pairs.
fn collect_loose_refs(dir: &Path, prefix: &str, git_dir: &Path, out: &mut Vec<(String, GitHash)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let refname = format!("{prefix}/{name}");
        let path = entry.path();
        if path.is_dir() {
            collect_loose_refs(&path, &refname, git_dir, out);
        } else if let Ok(hash) = resolve_ref(git_dir, &refname) {
            out.push((refname, hash));
        }
    }
}

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
        .map_err(|_| GitError::RefNotFound(format!("{refname}: invalid hash {content:?}")))
}
