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
