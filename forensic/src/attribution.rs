//! Attribution timeline — who did what, when, from which timezone.
//!
//! Every commit carries two identities: the **author** (who wrote the change)
//! and the **committer** (who applied it). This module flattens a set of commits
//! into a single time-ordered stream of identity events — the who-did-what-when
//! backbone an examiner builds a narrative on. The timezone offset is retained
//! because it can corroborate or contradict a claimed location.

use git_core::{CommitObject, GitHash, GitRepo, Result};

/// Which identity an attribution event came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
    /// The author — who wrote the change.
    Author = 0,
    /// The committer — who applied it to the repository.
    Committer = 1,
}

/// One identity event on the attribution timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributionEvent {
    /// The commit this identity is attached to.
    pub commit: GitHash,
    /// Author or committer.
    pub role: Role,
    /// Identity name.
    pub name: String,
    /// Identity email.
    pub email: String,
    /// Event time (epoch seconds).
    pub timestamp: i64,
    /// Timezone offset of the recorded time, in seconds east of UTC.
    pub tz_offset_secs: i32,
}

/// Build a time-ordered attribution timeline from a set of commits.
///
/// Each commit contributes two events (author, then committer). Events are
/// sorted by timestamp ascending; ties keep author before committer.
#[must_use]
pub fn attribution_timeline(commits: &[CommitObject]) -> Vec<AttributionEvent> {
    let mut events = Vec::with_capacity(commits.len() * 2);
    for c in commits {
        for (role, sig) in [(Role::Author, &c.author), (Role::Committer, &c.committer)] {
            events.push(AttributionEvent {
                commit: c.hash,
                role,
                name: sig.name.clone(),
                email: sig.email.clone(),
                timestamp: sig.timestamp,
                tz_offset_secs: sig.tz_offset_secs,
            });
        }
    }
    events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then((a.role as u8).cmp(&(b.role as u8))));
    events
}

/// Distinct `(name, email)` identities appearing across `commits`, in first-seen
/// order. A surprising count or unexpected identity is a lead, not a verdict.
#[must_use]
pub fn distinct_identities(commits: &[CommitObject]) -> Vec<(String, String)> {
    let mut seen = Vec::new();
    for c in commits {
        for sig in [&c.author, &c.committer] {
            let id = (sig.name.clone(), sig.email.clone());
            if !seen.contains(&id) {
                seen.push(id);
            }
        }
    }
    seen
}

/// Walk every commit reachable from `from` and build its attribution timeline.
///
/// # Errors
/// Propagates any [`git_core`] read error encountered while walking.
pub fn attribution_repo(repo: &GitRepo, from: GitHash) -> Result<Vec<AttributionEvent>> {
    let mut commits = Vec::new();
    for commit in repo.walk_commits(from) {
        commits.push(commit?);
    }
    Ok(attribution_timeline(&commits))
}

#[cfg(test)]
mod tests {
    use super::*;
    use git_core::Signature;

    fn sig(name: &str, ts: i64, tz: i32) -> Signature {
        Signature {
            name: name.into(),
            email: format!("{name}@x"),
            timestamp: ts,
            tz_offset_secs: tz,
        }
    }

    fn commit(hex: &str, author: Signature, committer: Signature) -> CommitObject {
        CommitObject {
            hash: GitHash::from_hex(hex).unwrap(),
            tree: GitHash::from_hex("89abcdef0123456789abcdef0123456789abcdef").unwrap(),
            parents: vec![],
            author,
            committer,
            message: "m".into(),
            is_signed: false,
        }
    }

    #[test]
    fn timeline_is_time_ordered_author_before_committer() {
        let c1 = commit(
            "0123456789abcdef0123456789abcdef01234567",
            sig("alice", 1_000, 0),
            sig("bob", 2_000, 3600),
        );
        let c2 = commit(
            "1123456789abcdef0123456789abcdef01234567",
            sig("carol", 1_500, -7200),
            sig("carol", 1_500, -7200),
        );
        let tl = attribution_timeline(&[c1, c2]);
        // 2 commits → 4 events, sorted by time: alice@1000, carol-author@1500,
        // carol-committer@1500, bob@2000.
        let times: Vec<i64> = tl.iter().map(|e| e.timestamp).collect();
        assert_eq!(times, vec![1_000, 1_500, 1_500, 2_000]);
        assert_eq!(tl[0].name, "alice");
        assert_eq!(tl[0].role, Role::Author);
        assert_eq!(tl[1].role, Role::Author); // carol author before committer at the tie
        assert_eq!(tl[2].role, Role::Committer);
        assert_eq!(tl[3].name, "bob");
        assert_eq!(tl[3].tz_offset_secs, 3600);
    }

    #[test]
    fn distinct_identities_dedup_in_first_seen_order() {
        let c = commit(
            "0123456789abcdef0123456789abcdef01234567",
            sig("alice", 1, 0),
            sig("bob", 2, 0),
        );
        let ids = distinct_identities(std::slice::from_ref(&c));
        assert_eq!(
            ids,
            vec![("alice".into(), "alice@x".into()), ("bob".into(), "bob@x".into())]
        );
        // committer == author → a single identity, no duplicate.
        let solo = commit(
            "1123456789abcdef0123456789abcdef01234567",
            sig("alice", 1, 0),
            sig("alice", 2, 0),
        );
        assert_eq!(distinct_identities(&[solo]).len(), 1);
    }
}
