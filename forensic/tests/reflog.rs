//! Reflog-residue analyzer validated against real `git`-produced repos.
//!
//! Git records the operation that moved a ref in its reflog message. A message
//! indicating `reset:`, `rebase`, `filter-branch`, `commit (amend)`, or a
//! `(forced update)` is consistent with history rewriting — a lead an examiner
//! follows, never a verdict.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::process::Command;

use git_core::GitRepo;
use git_forensic::{audit_reflog, ReflogAnomaly};

fn git(root: &Path, args: &[&str]) {
    let st = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "A")
        .env("GIT_AUTHOR_EMAIL", "a@b.x")
        .env("GIT_COMMITTER_NAME", "A")
        .env("GIT_COMMITTER_EMAIL", "a@b.x")
        .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z")
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?}");
}

fn init(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
}

#[test]
fn flags_a_real_reset_in_the_reflog() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init(root);
    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "first"]);
    std::fs::write(root.join("f"), b"2").unwrap();
    git(root, &["commit", "-qam", "second"]);
    // History rewrite: drop the most recent commit from the branch tip.
    git(root, &["reset", "--hard", "HEAD~1", "-q"]);

    let repo = GitRepo::open(root).unwrap();
    let found = audit_reflog(&repo, "HEAD").unwrap();
    assert_eq!(found.len(), 1, "exactly the reset entry must be flagged");
    let ReflogAnomaly::HistoryRewrite {
        operation, message, ..
    } = &found[0];
    assert_eq!(operation, "reset");
    assert!(message.starts_with("reset:"));
}

#[test]
fn flags_a_real_amend_in_the_reflog() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init(root);
    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "first"]);
    git(root, &["commit", "-q", "--amend", "-m", "first (reworded)"]);

    let repo = GitRepo::open(root).unwrap();
    let found = audit_reflog(&repo, "HEAD").unwrap();
    assert_eq!(found.len(), 1, "the amend entry must be flagged");
    let ReflogAnomaly::HistoryRewrite { operation, .. } = &found[0];
    assert_eq!(operation, "amend");
}

#[test]
fn plain_commits_are_not_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init(root);
    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "first"]);
    std::fs::write(root.join("f"), b"2").unwrap();
    git(root, &["commit", "-qam", "second"]);

    let repo = GitRepo::open(root).unwrap();
    let found = audit_reflog(&repo, "HEAD").unwrap();
    assert!(found.is_empty(), "ordinary commits are not rewrites");
}

#[test]
fn finding_carries_code_severity_category() {
    use forensicnomicon::report::{Category, Severity};
    use git_forensic::source;

    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    init(root);
    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "first"]);
    std::fs::write(root.join("f"), b"2").unwrap();
    git(root, &["commit", "-qam", "second"]);
    git(root, &["reset", "--hard", "HEAD~1", "-q"]);

    let repo = GitRepo::open(root).unwrap();
    let found = audit_reflog(&repo, "HEAD").unwrap();
    let f = found[0].to_finding(source("HEAD"));
    assert_eq!(f.code, "GIT-HISTORY-REWRITE");
    assert_eq!(f.severity, Some(Severity::Medium));
    assert_eq!(f.category, Category::History);
}
