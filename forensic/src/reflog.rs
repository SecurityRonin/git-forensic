//! Reflog-residue analysis: history-rewriting operations recorded in the reflog.
//!
//! Git stamps every ref movement into `.git/logs/<ref>` with a message naming
//! the operation that moved it (`commit:`, `reset:`, `rebase (finish):`,
//! `filter-branch:`, `commit (amend):`, `… (forced update)`). A message whose
//! operation rewrites history is residue an examiner follows — the original tip
//! is still resurrectable from the reflog and the object store — never a verdict.

use forensicnomicon::report::{Category, Evidence, Observation, Severity};
use git_core::{GitHash, GitRepo, ReflogEntry, Result};

/// A history-rewriting operation observed in a ref's reflog.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReflogAnomaly {
    /// A reflog entry whose operation rewrote history (reset/rebase/amend/…).
    HistoryRewrite {
        /// The ref whose reflog this entry belongs to (e.g. `HEAD`).
        ref_name: String,
        /// The ref's value before the rewrite.
        old: GitHash,
        /// The ref's value after the rewrite.
        new: GitHash,
        /// The classified operation keyword (`reset`, `rebase`, `amend`, …).
        operation: String,
        /// The full reflog message.
        message: String,
    },
}

impl ReflogAnomaly {
    /// The stable, published anomaly code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::HistoryRewrite { .. } => "GIT-HISTORY-REWRITE",
        }
    }
}

impl Observation for ReflogAnomaly {
    fn severity(&self) -> Option<Severity> {
        match self {
            // A rewrite is a routine developer action as often as it is an
            // attempt to bury history, so it is a Medium-grade lead, not High.
            Self::HistoryRewrite { .. } => Some(Severity::Medium),
        }
    }

    fn code(&self) -> &'static str {
        ReflogAnomaly::code(self)
    }

    fn category(&self) -> Category {
        // Rewriting a ref's past is a History signal.
        Category::History
    }

    fn note(&self) -> String {
        match self {
            Self::HistoryRewrite {
                ref_name,
                operation,
                ..
            } => format!(
                "reflog of {ref_name} records a {operation} operation; consistent \
                 with history rewriting (the prior tip remains resurrectable from \
                 the reflog and object store)"
            ),
        }
    }

    fn evidence(&self) -> Vec<Evidence> {
        match self {
            Self::HistoryRewrite {
                old, new, message, ..
            } => vec![
                Evidence {
                    field: "old".into(),
                    value: old.to_hex(),
                    location: None,
                },
                Evidence {
                    field: "new".into(),
                    value: new.to_hex(),
                    location: None,
                },
                Evidence {
                    field: "message".into(),
                    value: message.clone(),
                    location: None,
                },
            ],
        }
    }
}

/// Classify a reflog message as a history-rewriting operation, returning the
/// operation keyword if it is one (pure; the analyzer's decision core).
///
/// Recognized rewrites: `reset:`, any `rebase` variant, `filter-branch`,
/// `commit (amend)`, and a trailing `(forced update)` (e.g. a force-push).
#[must_use]
pub fn classify_rewrite(message: &str) -> Option<&'static str> {
    if message.starts_with("reset:") {
        Some("reset")
    } else if message.contains("commit (amend)") {
        Some("amend")
    } else if message.contains("filter-branch") {
        Some("filter-branch")
    } else if message.contains("rebase") {
        Some("rebase")
    } else if message.contains("(forced update)") {
        Some("forced-update")
    } else {
        None
    }
}

/// Audit a set of reflog entries for `ref_name`, flagging history rewrites
/// (pure; side-effect free).
#[must_use]
pub fn audit_reflog_entries(ref_name: &str, entries: &[ReflogEntry]) -> Vec<ReflogAnomaly> {
    entries
        .iter()
        .filter_map(|e| {
            classify_rewrite(&e.message).map(|operation| ReflogAnomaly::HistoryRewrite {
                ref_name: ref_name.to_string(),
                old: e.old,
                new: e.new,
                operation: operation.to_string(),
                message: e.message.clone(),
            })
        })
        .collect()
}

/// Read `refname`'s reflog from `repo` and audit it for history rewrites.
///
/// # Errors
/// Propagates any [`git_core`] error encountered reading the reflog.
pub fn audit_reflog(repo: &GitRepo, refname: &str) -> Result<Vec<ReflogAnomaly>> {
    let entries = repo.reflog(refname)?;
    Ok(audit_reflog_entries(refname, &entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_each_rewrite_kind() {
        assert_eq!(classify_rewrite("reset: moving to HEAD~1"), Some("reset"));
        assert_eq!(classify_rewrite("rebase (finish): refs/heads/x"), Some("rebase"));
        assert_eq!(classify_rewrite("rebase -i (start): x"), Some("rebase"));
        assert_eq!(classify_rewrite("filter-branch: rewrite"), Some("filter-branch"));
        assert_eq!(classify_rewrite("commit (amend): reworded"), Some("amend"));
        assert_eq!(
            classify_rewrite("update by push (forced update)"),
            Some("forced-update")
        );
    }

    #[test]
    fn does_not_flag_ordinary_operations() {
        assert!(classify_rewrite("commit: add feature").is_none());
        assert!(classify_rewrite("commit (initial): first").is_none());
        assert!(classify_rewrite("merge topic: Fast-forward").is_none());
        assert!(classify_rewrite("checkout: moving from a to b").is_none());
        assert!(classify_rewrite("clone: from https://x").is_none());
    }

    fn entry(message: &str) -> ReflogEntry {
        ReflogEntry {
            old: GitHash::from_hex("0123456789abcdef0123456789abcdef01234567").unwrap(),
            new: GitHash::from_hex("89abcdef0123456789abcdef0123456789abcdef").unwrap(),
            name: "A".into(),
            email: "a@b.x".into(),
            timestamp: 100,
            tz_offset: "+0000".into(),
            message: message.into(),
        }
    }

    #[test]
    fn audit_flags_only_rewrites() {
        let entries = vec![
            entry("commit (initial): first"),
            entry("commit: second"),
            entry("reset: moving to HEAD~1"),
        ];
        let found = audit_reflog_entries("HEAD", &entries);
        assert_eq!(found.len(), 1);
        let ReflogAnomaly::HistoryRewrite { operation, .. } = &found[0];
        assert_eq!(operation, "reset");
    }
}
