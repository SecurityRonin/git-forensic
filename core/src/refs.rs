use std::path::Path;

use crate::error::{GitError, Result};
use crate::hash::GitHash;

/// Enumerate every ref in `git_dir`: the loose `refs/**` tree, `packed-refs`,
/// and `HEAD` (when it resolves to a hash). Returns `(refname, target_hash)`
/// pairs. Unresolvable or malformed refs are skipped; never panics.
#[must_use]
pub fn list_refs(git_dir: &Path) -> Vec<(String, GitHash)> {
    // Best-effort facade: a refs-subsystem I/O failure degrades to fewer refs.
    // Reachability callers MUST use `list_refs_checked` instead — see its docs.
    list_refs_checked(git_dir).unwrap_or_default()
}

/// Like [`list_refs`] but **fails loud on a ref-enumeration I/O error** instead of
/// silently returning fewer refs. A genuine failure to read the refs subsystem
/// (e.g. `refs/` present but unreadable, a `packed-refs` that exists but can't be
/// read) is a *bootstrap* failure: callers that compute reachability from these
/// refs (the unreachable-object audit) must NOT treat it as "zero refs" — that
/// would flag every object as orphaned (a false-positive inversion). A genuinely
/// *absent* file/dir (`NotFound`) is the legitimate empty case and is NOT an error.
///
/// # Errors
/// [`GitError::Io`] if a refs path that exists cannot be enumerated/read.
pub fn list_refs_checked(git_dir: &Path) -> Result<Vec<(String, GitHash)>> {
    let mut out: Vec<(String, GitHash)> = Vec::new();

    // Loose refs under refs/ (recursively). An absent refs/ is the legitimate
    // empty case; any other read failure is a bootstrap error that propagates.
    let refs_root = git_dir.join("refs");
    collect_loose_refs_checked(&refs_root, "refs", git_dir, &mut out)?;

    // packed-refs: lines of "<sha> <refname>"; "^<sha>" peel lines are skipped
    // (the peeled tag commit is reachable via the tag object itself). Absent is
    // fine; an existing-but-unreadable packed-refs is a bootstrap failure.
    match std::fs::read_to_string(git_dir.join("packed-refs")) {
        Ok(text) => {
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
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(GitError::Io(e)),
    }

    // HEAD, resolved to a hash (symbolic or detached). A missing/unborn HEAD is
    // a per-ref miss (RefNotFound); only an I/O failure is a bootstrap error.
    match resolve_ref(git_dir, "HEAD") {
        Ok(hash) => {
            if !out.iter().any(|(n, _)| n == "HEAD") {
                out.push(("HEAD".to_string(), hash));
            }
        }
        Err(GitError::RefNotFound(_)) => {}
        Err(e) => return Err(e),
    }

    Ok(out)
}

/// Recursively walk a loose-ref directory, appending `(refname, hash)` pairs.
///
/// A genuinely-absent directory (`NotFound`) is the empty case and yields
/// `Ok(())`; any other `read_dir`/entry I/O error — including `refs/` existing
/// but not being a directory — propagates as [`GitError::Io`]. Individual refs
/// that fail to *resolve* (`RefNotFound`) are skipped as per-artifact misses,
/// not bootstrap failures.
fn collect_loose_refs_checked(
    dir: &Path,
    prefix: &str,
    git_dir: &Path,
    out: &mut Vec<(String, GitHash)>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(GitError::Io(e)),
    };
    for entry in entries {
        let entry = entry.map_err(GitError::Io)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let refname = format!("{prefix}/{name}");
        let path = entry.path();
        if path.is_dir() {
            collect_loose_refs_checked(&path, &refname, git_dir, out)?;
        } else {
            match resolve_ref(git_dir, &refname) {
                Ok(hash) => out.push((refname, hash)),
                Err(GitError::RefNotFound(_)) => {} // unresolvable ref: per-artifact miss
                Err(e) => return Err(e),            // I/O error reading an existing ref
            }
        }
    }
    Ok(())
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

#[cfg(test)]
mod refs_bootstrap_tests {
    use super::*;

    #[test]
    fn list_refs_checked_errs_on_unreadable_refs() {
        let tmp = tempfile::tempdir().unwrap();
        // Corrupt repo: `refs` is a FILE where a directory is expected, so
        // read_dir fails with a non-NotFound error. That I/O failure to enumerate
        // refs MUST surface — swallowing it into empty roots makes the unreachable
        // audit flag every object as orphaned (a false-positive flood).
        std::fs::write(tmp.path().join("refs"), b"corrupt").unwrap();
        assert!(
            list_refs_checked(tmp.path()).is_err(),
            "an unreadable refs/ must surface as an error, not empty refs"
        );
    }

    #[test]
    fn list_refs_checked_ok_when_refs_genuinely_absent() {
        let tmp = tempfile::tempdir().unwrap();
        // No refs/, no packed-refs, no HEAD → genuinely zero refs (a fresh/refless
        // repo). The legitimate empty case must be Ok(empty), NOT an error — else
        // we'd refuse on a genuinely all-orphaned repository.
        assert_eq!(list_refs_checked(tmp.path()).unwrap(), Vec::new());
    }
}
