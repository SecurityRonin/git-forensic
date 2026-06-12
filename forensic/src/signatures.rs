//! Signature-policy analysis: an unsigned commit in an otherwise-signed history.
//!
//! When a repository's reachable history is predominantly signed, the absence of
//! a signature on a particular commit is a break in the prevailing signing
//! policy — consistent with a commit injected or forged outside the normal
//! signed workflow. It is a lead an examiner follows, never a verdict: a
//! developer may simply have forgotten to sign.

use forensicnomicon::report::{Category, Evidence, Observation, Severity};
use git_core::{CommitObject, GitHash, GitRepo, Result};

/// A signing-policy anomaly observed across a set of commits.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignatureAnomaly {
    /// An unsigned commit within a predominantly-signed history.
    UnsignedInSignedHistory {
        /// The unsigned commit's object hash.
        commit: GitHash,
        /// How many commits in the audited set were signed.
        signed_count: usize,
        /// Total commits in the audited set.
        total_count: usize,
    },
}

impl SignatureAnomaly {
    /// The stable, published anomaly code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsignedInSignedHistory { .. } => "GIT-UNSIGNED-IN-SIGNED-HISTORY",
        }
    }
}

impl Observation for SignatureAnomaly {
    fn severity(&self) -> Option<Severity> {
        match self {
            // A real policy break, but a forgotten `-S` is a common benign cause,
            // so it is graded Medium, not High.
            Self::UnsignedInSignedHistory { .. } => Some(Severity::Medium),
        }
    }

    fn code(&self) -> &'static str {
        SignatureAnomaly::code(self)
    }

    fn category(&self) -> Category {
        // A break in cryptographic signing policy is an Integrity signal.
        Category::Integrity
    }

    fn note(&self) -> String {
        match self {
            Self::UnsignedInSignedHistory {
                signed_count,
                total_count,
                ..
            } => format!(
                "commit is unsigned while {signed_count} of {total_count} reachable \
                 commits are signed; consistent with a commit injected outside the \
                 prevailing signing policy (a forgotten signature is a benign cause)"
            ),
        }
    }

    fn evidence(&self) -> Vec<Evidence> {
        match self {
            Self::UnsignedInSignedHistory {
                commit,
                signed_count,
                total_count,
            } => vec![
                Evidence {
                    field: "commit".into(),
                    value: commit.to_hex(),
                    location: None,
                },
                Evidence {
                    field: "signed_count".into(),
                    value: signed_count.to_string(),
                    location: None,
                },
                Evidence {
                    field: "total_count".into(),
                    value: total_count.to_string(),
                    location: None,
                },
            ],
        }
    }
}

/// Audit a set of commits for the unsigned-in-signed-history anomaly (pure).
///
/// The set is "predominantly signed" when at least one commit is signed AND a
/// strict majority (`signed > total / 2`) are signed. Only then is an unsigned
/// commit anomalous; a fully- or mostly-unsigned history implies no signing
/// policy to break, so nothing is flagged.
#[must_use]
pub fn audit_signatures(commits: &[CommitObject]) -> Vec<SignatureAnomaly> {
    let total_count = commits.len();
    let signed_count = commits.iter().filter(|c| c.is_signed).count();

    // Predominantly signed: at least one signature and a strict majority.
    if signed_count == 0 || signed_count * 2 <= total_count {
        return Vec::new();
    }

    commits
        .iter()
        .filter(|c| !c.is_signed)
        .map(|c| SignatureAnomaly::UnsignedInSignedHistory {
            commit: c.hash,
            signed_count,
            total_count,
        })
        .collect()
}

/// Walk every commit reachable from `from` (first-parent) and audit their
/// signatures for the unsigned-in-signed-history anomaly.
///
/// # Errors
/// Propagates any [`git_core`] read error encountered while walking commits.
pub fn audit_signatures_repo(repo: &GitRepo, from: GitHash) -> Result<Vec<SignatureAnomaly>> {
    let mut commits = Vec::new();
    for commit in repo.walk_commits(from) {
        commits.push(commit?);
    }
    Ok(audit_signatures(&commits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::Signature;

    fn sig() -> Signature {
        Signature {
            name: "A".into(),
            email: "a@b.x".into(),
            timestamp: 100,
            tz_offset_secs: 0,
        }
    }

    fn commit(hex: &str, is_signed: bool) -> CommitObject {
        CommitObject {
            hash: GitHash::from_hex(hex).unwrap(),
            tree: GitHash::from_hex("89abcdef0123456789abcdef0123456789abcdef").unwrap(),
            parents: vec![],
            author: sig(),
            committer: sig(),
            message: "m".into(),
            is_signed,
        }
    }

    const A: &str = "0123456789abcdef0123456789abcdef01234567";
    const B: &str = "1123456789abcdef0123456789abcdef01234567";
    const C: &str = "2123456789abcdef0123456789abcdef01234567";

    #[test]
    fn flags_the_lone_unsigned_in_a_signed_majority() {
        let commits = vec![commit(A, true), commit(B, true), commit(C, false)];
        let found = audit_signatures(&commits);
        assert_eq!(found.len(), 1);
        let SignatureAnomaly::UnsignedInSignedHistory {
            commit,
            signed_count,
            total_count,
        } = &found[0];
        assert_eq!(commit.to_hex(), C);
        assert_eq!(*signed_count, 2);
        assert_eq!(*total_count, 3);
    }

    #[test]
    fn no_findings_when_all_signed() {
        let commits = vec![commit(A, true), commit(B, true)];
        assert!(audit_signatures(&commits).is_empty());
    }

    #[test]
    fn no_findings_when_all_unsigned() {
        let commits = vec![commit(A, false), commit(B, false)];
        assert!(audit_signatures(&commits).is_empty());
    }

    #[test]
    fn no_findings_without_a_signed_majority() {
        // Exactly half signed is not a strict majority → no policy in evidence.
        let commits = vec![commit(A, true), commit(B, false)];
        assert!(audit_signatures(&commits).is_empty());
    }

    #[test]
    fn empty_set_is_clean() {
        assert!(audit_signatures(&[]).is_empty());
    }
}
