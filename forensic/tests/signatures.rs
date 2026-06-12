//! Signature-policy analyzer validated against real `git`-produced commits.
//!
//! When a history is predominantly signed, an unsigned commit is a break in the
//! signing policy — consistent with an injected or forged commit, a lead an
//! examiner follows, never a verdict.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::process::Command;

use forensicnomicon::report::{Category, Observation, Severity};
use git_core::GitRepo;
use git_forensic::{audit_signatures, source, SignatureAnomaly};

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

fn ssh_keygen(root: &Path) -> String {
    let key = root.join("id");
    let st = Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-q", "-f"])
        .arg(&key)
        .status()
        .unwrap();
    assert!(st.success(), "ssh-keygen");
    format!("{}.pub", key.display())
}

#[test]
fn flags_unsigned_commit_in_signed_history() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let pubkey = ssh_keygen(root);
    git(root, &["init", "-q", "-b", "main"]);

    // Two signed commits, then one unsigned (signed majority).
    for (i, body) in [b"1".as_slice(), b"2".as_slice()].iter().enumerate() {
        std::fs::write(root.join("f"), body).unwrap();
        git(root, &["add", "f"]);
        git(
            root,
            &[
                "-c",
                "gpg.format=ssh",
                "-c",
                &format!("user.signingkey={pubkey}"),
                "commit",
                "-S",
                "-qm",
                &format!("signed {i}"),
            ],
        );
    }
    std::fs::write(root.join("f"), b"3").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "--no-gpg-sign", "-qm", "unsigned"]);

    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    let commits: Vec<_> = repo.walk_commits(head).map(|c| c.unwrap()).collect();

    let found = audit_signatures(&commits);
    assert_eq!(found.len(), 1, "exactly the unsigned commit must be flagged");
    let SignatureAnomaly::UnsignedInSignedHistory { commit, .. } = &found[0] else {
        panic!("expected UnsignedInSignedHistory")
    };
    assert_eq!(*commit, commits[0].hash, "the unsigned (newest) commit");

    let f = found[0].to_finding(source("repo"));
    assert_eq!(f.code, "GIT-UNSIGNED-IN-SIGNED-HISTORY");
    assert_eq!(f.severity, Some(Severity::Medium));
    assert_eq!(f.category, Category::Integrity);
}

#[test]
fn fully_unsigned_history_is_not_flagged() {
    // No signing policy in evidence → an unsigned commit is unremarkable.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    for body in [b"1".as_slice(), b"2".as_slice()] {
        std::fs::write(root.join("f"), body).unwrap();
        git(root, &["add", "f"]);
        git(root, &["commit", "-qm", "x"]);
    }

    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    let commits: Vec<_> = repo.walk_commits(head).map(|c| c.unwrap()).collect();
    assert!(audit_signatures(&commits).is_empty());
}
