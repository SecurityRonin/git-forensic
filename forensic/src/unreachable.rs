//! Reachability analysis: objects present in the store but reachable from no
//! ref. Such an object is residue of deleted or rewritten history — it remains
//! resurrectable from the object store until garbage-collected. Commits are the
//! most telling (a whole dropped line of development); blobs and trees are
//! lower-signal. An examiner follows these leads; they are never a verdict.

use std::collections::HashSet;

use forensicnomicon::report::{Category, Evidence, Observation, Severity};
use git_core::{GitHash, GitRepo, ObjectKind, Result};

/// An object present in the store yet reachable from no ref.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreachableObject {
    /// The unreachable object's hash.
    pub object: GitHash,
    /// Its kind as a lowercase git type (`commit`, `tree`, `blob`, `tag`).
    pub kind: String,
}

impl Observation for UnreachableObject {
    fn severity(&self) -> Option<Severity> {
        // A resurrectable dropped commit (deleted/rewritten history) is the
        // sharper lead; a loose blob/tree is lower signal.
        if self.kind == "commit" {
            Some(Severity::Medium)
        } else {
            Some(Severity::Low)
        }
    }

    fn code(&self) -> &'static str {
        "GIT-UNREACHABLE-OBJECT"
    }

    fn category(&self) -> Category {
        // Residue of deleted/rewritten history.
        Category::Residue
    }

    fn note(&self) -> String {
        format!(
            "{} object reachable from no ref; consistent with deleted or rewritten \
             history (the object remains resurrectable from the store until gc)",
            self.kind
        )
    }

    fn evidence(&self) -> Vec<Evidence> {
        vec![
            Evidence {
                field: "object".into(),
                value: self.object.to_hex(),
                location: None,
            },
            Evidence {
                field: "kind".into(),
                value: self.kind.clone(),
                location: None,
            },
        ]
    }
}

fn kind_str(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Commit => "commit",
        ObjectKind::Tree => "tree",
        ObjectKind::Blob => "blob",
        ObjectKind::Tag => "tag",
    }
}

/// Compute the set of objects reachable from every ref tip.
///
/// Walks commit → parents + tree, tree → entries (subtrees + blobs). A tag is
/// opaque (git-core does not parse tag targets), so a tag tip contributes only
/// itself; this is sound for the common branch/HEAD tips used in practice.
///
/// Objects that fail to read or parse are recorded as reached-but-not-expanded
/// rather than aborting the walk — robustness on a damaged store beats a hard
/// failure.
///
/// # Errors
/// Propagates a [`git_core::GitError::Io`] if the ref roots cannot be enumerated. This is
/// a *bootstrap* failure and MUST NOT be swallowed into an empty root set, which
/// would make every object look unreachable (a false-positive inversion).
fn reachable_set(repo: &GitRepo) -> Result<HashSet<GitHash>> {
    let mut reached = HashSet::new();
    let mut stack: Vec<GitHash> = repo
        .all_refs_checked()?
        .into_iter()
        .map(|(_, h)| h)
        .collect();

    while let Some(hash) = stack.pop() {
        if !reached.insert(hash) {
            continue;
        }
        let Ok(obj) = repo.read_object(&hash) else {
            continue;
        };
        match obj.kind {
            ObjectKind::Commit => {
                if let Ok(commit) = repo.read_commit(&hash) {
                    stack.push(commit.tree);
                    stack.extend(commit.parents);
                }
            }
            ObjectKind::Tree => {
                if let Ok(tree) = repo.read_tree(&hash) {
                    stack.extend(tree.entries.into_iter().map(|e| e.hash));
                }
            }
            // Blobs are leaves; tags are opaque here.
            ObjectKind::Blob | ObjectKind::Tag => {}
        }
    }
    Ok(reached)
}

/// Audit `repo` for objects reachable from no ref (`all_objects − reachable`).
///
/// # Errors
/// Propagates a [`git_core`] error from ref enumeration or object enumeration.
/// A ref-enumeration failure is surfaced rather than misreported as every
/// object being unreachable.
pub fn audit_unreachable(repo: &GitRepo) -> Result<Vec<UnreachableObject>> {
    let reached = reachable_set(repo)?;
    let mut out = Vec::new();
    for hash in repo.all_objects()? {
        if reached.contains(&hash) {
            continue;
        }
        // Determine the kind for grading; an unreadable object is reported as
        // "unknown" rather than dropped, so its presence is still surfaced.
        let kind = repo
            .read_object(&hash)
            .map_or("unknown", |o| kind_str(o.kind))
            .to_string();
        out.push(UnreachableObject { object: hash, kind });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_and_grading_depend_on_kind() {
        let commit = UnreachableObject {
            object: GitHash::from_hex("0123456789abcdef0123456789abcdef01234567").unwrap(),
            kind: "commit".into(),
        };
        let blob = UnreachableObject {
            object: GitHash::from_hex("89abcdef0123456789abcdef0123456789abcdef").unwrap(),
            kind: "blob".into(),
        };
        assert_eq!(commit.severity(), Some(Severity::Medium));
        assert_eq!(blob.severity(), Some(Severity::Low));
        assert_eq!(commit.code(), "GIT-UNREACHABLE-OBJECT");
        assert_eq!(commit.category(), Category::Residue);
        assert!(commit.note().contains("commit"));
    }

    /// The inversion guard: when ref enumeration FAILS (a bootstrap failure), the
    /// audit must ERROR, not silently treat the empty root set as "everything is
    /// unreachable" (which would flag every object in the store as orphaned — a
    /// false-positive flood indistinguishable from a genuinely all-orphaned repo).
    #[test]
    fn audit_unreachable_errs_when_ref_bootstrap_fails() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("HEAD"), b"ref: refs/heads/main\n").unwrap();
        std::fs::create_dir(tmp.path().join("objects")).unwrap();
        // Corrupt the refs subsystem: `refs` is a FILE, so enumeration fails.
        std::fs::write(tmp.path().join("refs"), b"corrupt").unwrap();
        let repo = GitRepo::open(tmp.path()).unwrap();
        assert!(
            audit_unreachable(&repo).is_err(),
            "a failed ref bootstrap must error, not report all objects unreachable"
        );
    }
}
