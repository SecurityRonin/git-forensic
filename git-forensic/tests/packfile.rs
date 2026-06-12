//! Packfile reading, validated against the real `git` binary as oracle.
//!
//! A repo built then `git repack`-ed stores its objects in a packfile (the
//! normal post-`gc`/clone state). These tests assert `read_object` returns the
//! same bytes `git cat-file` does — for whole objects AND delta-encoded ones.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use git_forensic::{GitHash, GitRepo, ObjectKind};

/// Build a repo whose `git repack` produces both whole and delta objects, then
/// pack everything into a single packfile (no loose objects remain).
fn packed_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@e.x"]);
    git(root, &["config", "user.name", "T"]);
    git(root, &["config", "commit.gpgsign", "false"]);

    std::fs::write(root.join("a.txt"), b"hello packfile forensics\n").unwrap();
    // big + a near-copy so the packer delta-encodes one against the other.
    let mut big = String::new();
    for i in 1..=30 {
        big.push_str(&format!("line {i} the quick brown fox\n"));
    }
    std::fs::write(root.join("big.txt"), &big).unwrap();
    std::fs::write(root.join("big2.txt"), format!("{big}line 31 extra\n")).unwrap();

    git(root, &["add", "-A"]);
    git_env_dated(root, &["commit", "-qm", "c1"]);
    // Local repack defaults to OFS_DELTA (type 6).
    git(root, &["repack", "-a", "-d", "-q", "--window=10", "--depth=10"]);
    dir
}

/// A packed repo forced to use REF_DELTA (type 7) — the 20-byte-base-hash form —
/// so that delta path is validated, not just OFS_DELTA.
fn packed_repo_ref_delta() -> TempDir {
    let dir = packed_repo();
    let root = dir.path();
    git(root, &["-c", "pack.useOfsDelta=false", "repack", "-a", "-d", "-q", "-f",
        "--window=10", "--depth=10"]);
    dir
}

fn git(root: &Path, args: &[&str]) {
    let st = Command::new("git").args(args).current_dir(root).status().unwrap();
    assert!(st.success(), "git {args:?}");
}

fn git_env_dated(root: &Path, args: &[&str]) {
    let st = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_AUTHOR_DATE", "2024-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2024-01-01T00:00:00Z")
        .env("GIT_AUTHOR_NAME", "T")
        .env("GIT_AUTHOR_EMAIL", "t@e.x")
        .env("GIT_COMMITTER_NAME", "T")
        .env("GIT_COMMITTER_EMAIL", "t@e.x")
        .status()
        .unwrap();
    assert!(st.success(), "git {args:?}");
}

/// `git` stdout, trimmed of a single trailing newline only when present as raw.
fn git_out(root: &Path, args: &[&str]) -> Vec<u8> {
    let out = Command::new("git").args(args).current_dir(root).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    out.stdout
}

fn rev_parse(root: &Path, rev: &str) -> GitHash {
    let s = String::from_utf8(git_out(root, &["rev-parse", rev])).unwrap();
    GitHash::from_hex(s.trim()).expect("hash")
}

#[test]
fn reads_packed_blob_matching_git() {
    let tr = packed_repo();
    let root = tr.path();
    let hash = rev_parse(root, "HEAD:a.txt");
    let got = GitRepo::open(root).unwrap().read_blob(&hash).expect("read packed blob");
    assert_eq!(got, git_out(root, &["cat-file", "-p", &hash.to_hex()]));
}

#[test]
fn reads_packed_commit_and_tree() {
    let tr = packed_repo();
    let root = tr.path();
    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    let commit = repo.read_object(&head).expect("read packed commit");
    assert_eq!(commit.kind, ObjectKind::Commit);
    assert!(commit.verified, "packed commit SHA1 must verify");
    let tree_hash = rev_parse(root, "HEAD^{tree}");
    let tree = repo.read_object(&tree_hash).expect("read packed tree");
    assert_eq!(tree.kind, ObjectKind::Tree);
    assert!(tree.verified);
}

#[test]
fn reads_delta_encoded_object_matching_git() {
    // big2.txt is stored as a delta against big.txt in the packfile; resolving it
    // exercises the OFS/REF-delta path. Its bytes must still match git's.
    let tr = packed_repo();
    let root = tr.path();
    let repo = GitRepo::open(root).unwrap();
    for name in ["big.txt", "big2.txt"] {
        let hash = rev_parse(root, &format!("HEAD:{name}"));
        let got = repo.read_blob(&hash).unwrap_or_else(|e| panic!("read {name}: {e}"));
        assert_eq!(got, git_out(root, &["cat-file", "-p", &hash.to_hex()]), "{name}");
        assert!(!got.is_empty());
    }
}

#[test]
fn reads_ref_delta_object_matching_git() {
    // Forces REF_DELTA (type 7): the delta names its base by 20-byte hash rather
    // than a back-offset, exercising resolve_base().
    let tr = packed_repo_ref_delta();
    let root = tr.path();
    let repo = GitRepo::open(root).unwrap();
    for name in ["big.txt", "big2.txt"] {
        let hash = rev_parse(root, &format!("HEAD:{name}"));
        let got = repo.read_blob(&hash).unwrap_or_else(|e| panic!("ref-delta {name}: {e}"));
        assert_eq!(got, git_out(root, &["cat-file", "-p", &hash.to_hex()]), "{name}");
    }
}

#[test]
fn packed_object_sha1_verifies() {
    let tr = packed_repo();
    let root = tr.path();
    let repo = GitRepo::open(root).unwrap();
    let hash = rev_parse(root, "HEAD:big2.txt");
    let obj = repo.read_object(&hash).expect("read delta object");
    assert!(obj.verified, "resolved delta object must hash back to its name");
}

#[test]
fn missing_object_in_packed_repo_still_errors() {
    let tr = packed_repo();
    let root = tr.path();
    let fake = GitHash::from_hex("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef").unwrap();
    assert!(GitRepo::open(root).unwrap().read_object(&fake).is_err());
}
