#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Commit-signature detection validated against real `git`-produced commits.
//!
//! A signed commit carries a `gpgsig <signature>` header between `committer`
//! and the blank line, its continuation lines prefixed with a single space.
//! `CommitObject::is_signed` must reflect the header's presence without
//! corrupting the message (everything after the first blank line).

use std::path::Path;
use std::process::Command;

use git_core::GitRepo;

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

/// Generate a passphraseless SSH key and return its public-key path.
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
fn detects_signed_and_unsigned_commits() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let pubkey = ssh_keygen(root);

    git(root, &["init", "-q", "-b", "main"]);

    // Two SSH-signed commits.
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

    // One explicitly-unsigned commit.
    std::fs::write(root.join("f"), b"3").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "--no-gpg-sign", "-qm", "unsigned"]);

    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    let commits: Vec<_> = repo.walk_commits(head).map(|c| c.unwrap()).collect();
    assert_eq!(commits.len(), 3);

    // Newest-first: unsigned, signed 1, signed 0.
    assert!(!commits[0].is_signed, "the --no-gpg-sign commit is unsigned");
    assert!(commits[0].message.starts_with("unsigned"));
    assert!(commits[1].is_signed, "signed commit must be detected");
    assert!(commits[1].message.starts_with("signed 1"));
    assert!(commits[2].is_signed, "signed commit must be detected");
    assert!(commits[2].message.starts_with("signed 0"));

    // Oracle cross-check: git itself agrees the unsigned commit has no signature.
    let out = Command::new("git")
        .args(["verify-commit", &commits[0].hash.to_hex()])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(!out.status.success(), "git verify-commit must fail on unsigned");
}
