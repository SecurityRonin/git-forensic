//! # git-forensic
//!
//! Forensic anomaly auditor for Git object stores, built on [`git_core`]. It
//! reads commits via the reader and emits graded
//! [`forensicnomicon::report::Finding`]s — observations, never legal
//! conclusions; the analyst draws the conclusion.
//!
//! First finding: **commit-time inversion** — a commit whose committer
//! timestamp precedes its author timestamp. In a normal flow the committer time
//! is at or after the author time, so an inversion is consistent with timestamp
//! backdating (benign causes include cross-machine clock skew).

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use forensicnomicon::report::{Category, Evidence, Observation, Severity, Source};
use git_core::{CommitObject, GitHash, GitRepo, Result};

/// A forensic anomaly observed in a Git object store.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GitAnomaly {
    /// A commit's committer timestamp precedes its author timestamp.
    CommitterBeforeAuthor {
        /// The commit's object hash.
        commit: GitHash,
        /// Author timestamp (epoch seconds).
        author_time: i64,
        /// Committer timestamp (epoch seconds).
        committer_time: i64,
    },
}

impl GitAnomaly {
    /// The stable, published anomaly code (scheme-prefixed SCREAMING-KEBAB).
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::CommitterBeforeAuthor { .. } => "GIT-COMMIT-TIME-INVERSION",
        }
    }
}

impl Observation for GitAnomaly {
    fn severity(&self) -> Option<Severity> {
        match self {
            // An inversion is a real irregularity but has a common benign cause
            // (clock skew), so it is graded Medium, not High.
            Self::CommitterBeforeAuthor { .. } => Some(Severity::Medium),
        }
    }

    fn code(&self) -> &'static str {
        GitAnomaly::code(self)
    }

    fn category(&self) -> Category {
        // The commit's temporal biography — backdating is a History signal.
        Category::History
    }

    fn note(&self) -> String {
        match self {
            Self::CommitterBeforeAuthor {
                committer_time,
                author_time,
                ..
            } => format!(
                "committer timestamp {committer_time} precedes author timestamp \
                 {author_time}; consistent with timestamp backdating (benign \
                 causes include cross-machine clock skew)"
            ),
        }
    }

    fn evidence(&self) -> Vec<Evidence> {
        match self {
            Self::CommitterBeforeAuthor {
                commit,
                author_time,
                committer_time,
            } => vec![
                Evidence {
                    field: "commit".into(),
                    value: commit.to_hex(),
                    location: None,
                },
                Evidence {
                    field: "author_time".into(),
                    value: author_time.to_string(),
                    location: None,
                },
                Evidence {
                    field: "committer_time".into(),
                    value: committer_time.to_string(),
                    location: None,
                },
            ],
        }
    }
}

/// Audit a single parsed commit for anomalies (pure; side-effect free).
#[must_use]
pub fn audit_commit(commit: &CommitObject) -> Vec<GitAnomaly> {
    let mut out = Vec::new();
    if commit.committer.timestamp < commit.author.timestamp {
        out.push(GitAnomaly::CommitterBeforeAuthor {
            commit: commit.hash,
            author_time: commit.author.timestamp,
            committer_time: commit.committer.timestamp,
        });
    }
    out
}

/// Audit every commit reachable from `from` (first-parent walk) in `repo`.
///
/// # Errors
/// Propagates any [`git_core`] read error encountered while walking commits.
pub fn audit_repo(repo: &GitRepo, from: GitHash) -> Result<Vec<GitAnomaly>> {
    let mut out = Vec::new();
    for commit in repo.walk_commits(from) {
        out.extend(audit_commit(&commit?));
    }
    Ok(out)
}

/// The [`Source`] stamp for findings this analyzer emits.
#[must_use]
pub fn source(scope: impl Into<String>) -> Source {
    Source {
        analyzer: "git-forensic".to_string(),
        scope: scope.into(),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::Signature;

    fn sig(ts: i64) -> Signature {
        Signature {
            name: "A".into(),
            email: "a@b.x".into(),
            timestamp: ts,
            tz_offset_secs: 0,
        }
    }

    fn commit(author_time: i64, committer_time: i64) -> CommitObject {
        CommitObject {
            hash: GitHash::from_hex("0123456789abcdef0123456789abcdef01234567").unwrap(),
            tree: GitHash::from_hex("89abcdef0123456789abcdef0123456789abcdef").unwrap(),
            parents: vec![],
            author: sig(author_time),
            committer: sig(committer_time),
            message: "m".into(),
        }
    }

    #[test]
    fn flags_committer_before_author() {
        let anomalies = audit_commit(&commit(1_000, 900)); // committed "before" authored
        assert_eq!(anomalies.len(), 1);
        assert!(matches!(
            anomalies[0],
            GitAnomaly::CommitterBeforeAuthor { .. }
        ));
    }

    #[test]
    fn normal_commit_has_no_anomaly() {
        assert!(audit_commit(&commit(1_000, 1_000)).is_empty()); // committer == author
        assert!(audit_commit(&commit(1_000, 1_050)).is_empty()); // committer after author
    }

    #[test]
    fn finding_carries_code_severity_category() {
        let a = audit_commit(&commit(1_000, 900));
        let f = a[0].to_finding(source("repo"));
        assert_eq!(f.code, "GIT-COMMIT-TIME-INVERSION");
        assert_eq!(f.severity, Some(Severity::Medium));
        assert_eq!(f.category, Category::History);
        assert_eq!(f.source.analyzer, "git-forensic");
    }
}
