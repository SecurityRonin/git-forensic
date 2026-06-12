//! Analyzer validated against real `git`-produced repos.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::process::Command;

use git_core::GitRepo;
use git_forensic::{audit_repo, GitAnomaly};

fn git(root: &Path, args: &[&str], author: &str, committer: &str) {
    let st = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "A")
        .env("GIT_AUTHOR_EMAIL", "a@b.x")
        .env("GIT_COMMITTER_NAME", "A")
        .env("GIT_COMMITTER_EMAIL", "a@b.x")
        .env("GIT_AUTHOR_DATE", author)
        .env("GIT_COMMITTER_DATE", committer)
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?}");
}

fn init(root: &Path) {
    let d = ("2020-01-01T00:00:00Z", "2020-01-01T00:00:00Z");
    git(root, &["init", "-q", "-b", "main"], d.0, d.1);
    git(root, &["config", "commit.gpgsign", "false"], d.0, d.1);
}

#[test]
fn flags_a_real_backdated_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init(root);
    std::fs::write(root.join("f"), b"x").unwrap();
    git(root, &["add", "f"], "", "");
    // committer date EARLIER than author date → time inversion (backdating).
    git(
        root,
        &["commit", "-qm", "backdated"],
        "2020-06-01T00:00:00Z",
        "2020-05-01T00:00:00Z",
    );

    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    let anomalies = audit_repo(&repo, head).unwrap();
    assert_eq!(anomalies.len(), 1, "the backdated commit must be flagged");
    assert!(matches!(
        anomalies[0],
        GitAnomaly::CommitterBeforeAuthor { .. }
    ));
}

#[test]
fn clean_repo_produces_no_findings() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init(root);
    std::fs::write(root.join("f"), b"x").unwrap();
    git(root, &["add", "f"], "", "");
    git(
        root,
        &["commit", "-qm", "ok"],
        "2020-05-01T00:00:00Z",
        "2020-05-01T00:00:00Z",
    ); // committer == author

    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    assert!(audit_repo(&repo, head).unwrap().is_empty());
}
