#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Reflog reader validated against real `git`-produced logs.
//!
//! The reflog (`.git/logs/<ref>`) records every movement of a ref. Each line is
//! `<oldsha40> <newsha40> <name> <email> <unix_ts> <tzoffset>\t<message>\n`.

use std::path::Path;
use std::process::Command;

use git_core::reflog::{parse_reflog, ReflogEntry};
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

#[test]
fn parse_reflog_extracts_fields() {
    let line = b"0000000000000000000000000000000000000000 \
3abc579ce97f2484371fbe6e52d1fa43699479b5 A <a@b.x> 1781281480 +0800\tcommit (initial): first\n";
    let entries = parse_reflog(line);
    assert_eq!(entries.len(), 1);
    let e: &ReflogEntry = &entries[0];
    assert_eq!(e.old.to_hex(), "0000000000000000000000000000000000000000");
    assert_eq!(e.new.to_hex(), "3abc579ce97f2484371fbe6e52d1fa43699479b5");
    assert_eq!(e.name, "A");
    assert_eq!(e.email, "a@b.x");
    assert_eq!(e.timestamp, 1_781_281_480);
    assert_eq!(e.tz_offset, "+0800");
    assert_eq!(e.message, "commit (initial): first");
}

#[test]
fn parse_reflog_handles_name_with_spaces() {
    let line = b"0000000000000000000000000000000000000000 \
3abc579ce97f2484371fbe6e52d1fa43699479b5 Ada Lovelace <ada@x> 100 -0500\tcommit: x\n";
    let entries = parse_reflog(line);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Ada Lovelace");
    assert_eq!(entries[0].email, "ada@x");
    assert_eq!(entries[0].tz_offset, "-0500");
}

#[test]
fn parse_reflog_skips_malformed_lines() {
    // A line with no tab, a too-short line, and a valid line.
    let bytes = b"garbage with no tab\n\
short\tnope\n\
0000000000000000000000000000000000000000 \
3abc579ce97f2484371fbe6e52d1fa43699479b5 A <a@b.x> 100 +0000\tok\n";
    let entries = parse_reflog(bytes);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].message, "ok");
}

#[test]
fn reflog_missing_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"]);
    let repo = GitRepo::open(root).unwrap();
    // No ref movements yet for a never-written ref → empty, not an error.
    let entries = repo.reflog("refs/heads/does-not-exist").unwrap();
    assert!(entries.is_empty());
}

#[test]
fn reflog_reads_head_from_real_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    std::fs::write(root.join("f"), b"1").unwrap();
    git(root, &["add", "f"]);
    git(root, &["commit", "-qm", "first"]);
    std::fs::write(root.join("f"), b"2").unwrap();
    git(root, &["commit", "-qam", "second"]);
    git(root, &["reset", "--hard", "HEAD~1", "-q"]);

    let repo = GitRepo::open(root).unwrap();
    let entries = repo.reflog("HEAD").unwrap();
    // first commit, second commit, reset.
    assert_eq!(entries.len(), 3);
    assert!(entries[0].message.starts_with("commit (initial):"));
    assert!(entries[1].message.starts_with("commit:"));
    assert!(entries[2].message.starts_with("reset:"));
}
