//! Attribution timeline validated against a real git repo.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::process::Command;

use git_core::GitRepo;
use git_forensic::attribution::{attribution_repo, Role};

fn git(root: &Path, args: &[&str], name: &str, when: &str) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", name)
        .env("GIT_AUTHOR_EMAIL", format!("{name}@x"))
        .env("GIT_COMMITTER_NAME", name)
        .env("GIT_COMMITTER_EMAIL", format!("{name}@x"))
        .env("GIT_AUTHOR_DATE", when)
        .env("GIT_COMMITTER_DATE", when)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?}");
}

#[test]
fn timeline_from_a_real_two_author_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"], "x", "2020-01-01T00:00:00Z");
    git(root, &["config", "commit.gpgsign", "false"], "x", "2020-01-01T00:00:00Z");

    std::fs::write(root.join("a"), b"1").unwrap();
    git(root, &["add", "a"], "alice", "2020-01-01T00:00:00Z");
    git(root, &["commit", "-qm", "c1"], "alice", "2020-01-01T00:00:00Z");
    std::fs::write(root.join("b"), b"2").unwrap();
    git(root, &["add", "b"], "bob", "2020-02-01T00:00:00Z");
    git(root, &["commit", "-qm", "c2"], "bob", "2020-02-01T00:00:00Z");

    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    let tl = attribution_repo(&repo, head).unwrap();

    // 2 commits → 4 events, time-ordered, oldest first.
    assert_eq!(tl.len(), 4);
    assert!(tl.windows(2).all(|w| w[0].timestamp <= w[1].timestamp), "time-ordered");
    assert_eq!(tl[0].name, "alice");
    assert_eq!(tl[0].role, Role::Author);
    assert_eq!(tl.last().unwrap().name, "bob");
    // distinct identities across the history
    let names: std::collections::BTreeSet<_> = tl.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["alice", "bob"].into_iter().collect());
}
