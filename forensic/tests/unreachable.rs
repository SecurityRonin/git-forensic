//! Unreachable-object analyzer validated against real `git fsck`.
//!
//! An object present in the store but reachable from no ref is residue of
//! deleted or rewritten history — still resurrectable. We cross-check our
//! commit verdicts against `git fsck --unreachable --no-reflogs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use forensicnomicon::report::{Category, Observation};
use git_core::GitRepo;
use git_forensic::{audit_unreachable, source, UnreachableObject};

fn git(root: &Path, args: &[&str]) -> std::process::Output {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_NAME", "A")
        .env("GIT_AUTHOR_EMAIL", "a@b.x")
        .env("GIT_COMMITTER_NAME", "A")
        .env("GIT_COMMITTER_EMAIL", "a@b.x")
        .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}");
    out
}

/// `git fsck`'s unreachable *commit* set (the cross-check oracle).
fn fsck_unreachable_commits(root: &Path) -> BTreeSet<String> {
    let out = git(root, &["fsck", "--unreachable", "--no-reflogs"]);
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("unreachable commit "))
        .map(str::to_string)
        .collect()
}

#[test]
fn flags_an_orphaned_commit_and_agrees_with_fsck() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    // Disable reflogs so the dropped commit is genuinely unreachable.
    git(root, &["config", "core.logAllRefUpdates", "false"]);

    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "A"]);
    std::fs::write(root.join("f"), b"2").unwrap();
    git(root, &["commit", "-qam", "B"]);
    let b_sha = String::from_utf8(git(root, &["rev-parse", "HEAD"]).stdout)
        .unwrap()
        .trim()
        .to_string();
    // Orphan commit B: drop it from the branch tip.
    git(root, &["reset", "--hard", "HEAD~1", "-q"]);

    let repo = GitRepo::open(root).unwrap();
    let found = audit_unreachable(&repo).unwrap();

    // Our unreachable-commit set must contain B.
    let our_commits: BTreeSet<String> = found.iter().map(|a| a.object.to_hex()).collect();
    assert!(our_commits.contains(&b_sha), "orphaned commit B must be flagged");

    // Cross-check our unreachable *commits* against git fsck.
    let our_unreachable_commits: BTreeSet<String> = found
        .iter()
        .filter(|a| a.kind == "commit")
        .map(|a| a.object.to_hex())
        .collect();
    let theirs = fsck_unreachable_commits(root);
    assert_eq!(
        our_unreachable_commits, theirs,
        "unreachable-commit verdicts must agree with git fsck"
    );

    // Grading: a commit finding is in the Residue category.
    let f = found[0].to_finding(source("repo"));
    assert_eq!(f.code, "GIT-UNREACHABLE-OBJECT");
    assert_eq!(f.category, Category::Residue);
}

#[test]
fn clean_repo_has_no_unreachable_objects() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    git(root, &["config", "core.logAllRefUpdates", "false"]);
    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "A"]);

    let repo = GitRepo::open(root).unwrap();
    assert!(
        audit_unreachable(&repo).unwrap().is_empty(),
        "every object is reachable from HEAD"
    );
}
