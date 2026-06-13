#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Object + ref enumeration validated against real `git`-produced repos.
//!
//! `all_objects` must equal the union of loose and packed objects; `all_refs`
//! must equal what `git show-ref` / `HEAD` report.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use git_core::loose::list_loose;
use git_core::pack::list_packed;
use git_core::refs::list_refs;
use git_core::{GitHash, GitRepo};

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

fn build_repo(root: &Path) {
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "A"]);
    std::fs::write(root.join("g"), b"2").unwrap();
    git(root, &["add", "g"]);
    git(root, &["commit", "-qm", "B"]);
}

// ── cross-platform (.git produced on / read from mac, linux, windows) ──────────

#[test]
fn nested_ref_name_is_slash_canonical_on_every_host() {
    // A nested branch (refs/heads/feature/sub) is stored as a directory tree on
    // disk. The ref NAME must always be git-canonical with '/' separators,
    // regardless of the analysis host's path separator — collect_loose_refs joins
    // single file_name() components with '/', never an OS path. This is the core
    // of "reads a .git from any OS on any OS".
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    build_repo(root);
    git(root, &["branch", "feature/sub"]);

    let refs = list_refs(&root.join(".git"));
    assert!(
        refs.iter().any(|(n, _)| n == "refs/heads/feature/sub"),
        "nested ref must be slash-canonical, got: {:?}",
        refs.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
    );
    // And never leak a backslash (the Windows separator) into a ref name.
    assert!(
        refs.iter().all(|(n, _)| !n.contains('\\')),
        "no ref name may contain a backslash"
    );
}

#[test]
fn crlf_packed_refs_parse_like_a_windows_authored_git() {
    // git on Windows can leave CRLF line endings in control files. A packed-refs
    // written with \r\n must parse identically to \n — the reader splits on
    // .lines() (which strips a trailing \r) and trims each line.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    build_repo(root);
    let git_dir = root.join(".git");
    let head = GitRepo::open(root).unwrap().head().unwrap().to_hex();

    // Overwrite packed-refs with explicit CRLF, including a comment + a peel line.
    let packed = format!("# pack-refs with: peeled fully-peeled sorted\r\n{head} refs/heads/crlf-branch\r\n");
    std::fs::write(git_dir.join("packed-refs"), packed.as_bytes()).unwrap();

    let refs = list_refs(&git_dir);
    let found = refs.iter().find(|(n, _)| n == "refs/heads/crlf-branch");
    assert!(found.is_some(), "CRLF packed-refs line must parse");
    assert_eq!(
        found.unwrap().1.to_hex(),
        head,
        "CRLF ref must resolve to the right hash (no trailing \\r in the name or sha)"
    );
}

/// Every object git knows (`git cat-file --batch-all-objects`), as a set.
fn all_git_objects(root: &Path) -> BTreeSet<String> {
    let out = git(
        root,
        &["cat-file", "--batch-all-objects", "--batch-check=%(objectname)"],
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn hex_set(hashes: &[GitHash]) -> BTreeSet<String> {
    hashes.iter().map(GitHash::to_hex).collect()
}

#[test]
fn all_objects_matches_git_when_loose() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_repo(root);

    let repo = GitRepo::open(root).unwrap();
    let ours = hex_set(&repo.all_objects().unwrap());
    let theirs = all_git_objects(root);
    assert_eq!(ours, theirs, "loose enumeration must match git");
}

#[test]
fn all_objects_matches_git_when_packed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_repo(root);
    git(root, &["repack", "-a", "-d", "-q"]); // everything packed, loose dropped

    let repo = GitRepo::open(root).unwrap();
    let ours = hex_set(&repo.all_objects().unwrap());
    let theirs = all_git_objects(root);
    assert_eq!(ours, theirs, "packed enumeration must match git");
}

#[test]
fn list_loose_and_list_packed_split_correctly() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_repo(root);

    let objects_dir = root.join(".git").join("objects");
    let loose = list_loose(&objects_dir);
    let packed = list_packed(&objects_dir).unwrap();
    assert!(!loose.is_empty(), "fresh repo has loose objects");
    assert!(packed.is_empty(), "fresh repo has no packs");

    git(root, &["repack", "-a", "-d", "-q"]);
    let loose2 = list_loose(&objects_dir);
    let packed2 = list_packed(&objects_dir).unwrap();
    assert!(loose2.is_empty(), "after repack -d, no loose objects remain");
    assert!(!packed2.is_empty(), "after repack, objects are packed");
}

#[test]
fn all_refs_includes_head_target_and_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_repo(root);

    let repo = GitRepo::open(root).unwrap();
    let head = repo.head().unwrap();
    let refs = repo.all_refs();

    // The branch must be present and point at HEAD.
    let main = refs
        .iter()
        .find(|(name, _)| name == "refs/heads/main")
        .expect("refs/heads/main present");
    assert_eq!(main.1, head);
}

#[test]
fn list_refs_reads_packed_refs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_repo(root);
    git(root, &["pack-refs", "--all"]); // move loose refs into packed-refs

    let git_dir = root.join(".git");
    let refs = list_refs(&git_dir);
    assert!(
        refs.iter().any(|(name, _)| name == "refs/heads/main"),
        "packed-refs branch must be enumerated"
    );
}
