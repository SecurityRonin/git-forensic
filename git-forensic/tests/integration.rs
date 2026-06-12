//! Integration tests for git-forensic.
//!
//! Fixtures are created with the real `git` binary in a temp directory, ensuring
//! the parser is tested against objects produced by an independent tool
//! (doer-checker principle).

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use git_forensic::{GitHash, GitRepo, ObjectKind};

// ── fixture builder ──────────────────────────────────────────────────────────

struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    /// Create a minimal 2-commit git repo with:
    ///   commit 1: hello.txt = "hello forensics\n"
    ///   commit 2: hello.txt updated + world.txt = "world\n"
    fn new_two_commit() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        fn git(root: &Path, args: &[&str]) {
            let status = Command::new("git")
                .args(args)
                .current_dir(root)
                .env("GIT_AUTHOR_NAME", "Forensic Tester")
                .env("GIT_AUTHOR_EMAIL", "tester@forensic.example")
                .env("GIT_COMMITTER_NAME", "Forensic Tester")
                .env("GIT_COMMITTER_EMAIL", "tester@forensic.example")
                .env("GIT_AUTHOR_DATE", "2024-01-15T08:00:00+0800")
                .env("GIT_COMMITTER_DATE", "2024-01-15T08:00:00+0800")
                // Disable gitsign and GPG signing — test repos don't need Sigstore.
                .env("GIT_CONFIG_COUNT", "2")
                .env("GIT_CONFIG_KEY_0", "commit.gpgsign")
                .env("GIT_CONFIG_VALUE_0", "false")
                .env("GIT_CONFIG_KEY_1", "tag.gpgsign")
                .env("GIT_CONFIG_VALUE_1", "false")
                .status()
                .expect("git command failed");
            assert!(status.success(), "git {args:?} failed");
        }

        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "tester@forensic.example"]);
        git(root, &["config", "user.name", "Forensic Tester"]);

        std::fs::write(root.join("hello.txt"), b"hello forensics\n").unwrap();
        git(root, &["add", "hello.txt"]);
        git(root, &["commit", "-m", "initial commit"]);

        std::fs::write(root.join("hello.txt"), b"hello updated\n").unwrap();
        std::fs::write(root.join("world.txt"), b"world\n").unwrap();
        git(root, &["add", "hello.txt", "world.txt"]);
        git(root, &["commit", "-m", "second commit"]);

        Self { dir }
    }

    fn repo(&self) -> GitRepo {
        GitRepo::open(self.dir.path()).expect("GitRepo::open")
    }
}

/// Run a git subcommand in `root`, returning trimmed stdout.
fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git command failed");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn packed_object_reports_packfile_not_a_misleading_not_found() {
    // A real packed object: `git repack` moves loose objects into a packfile, the
    // normal state of any repo touched by `git gc`/clone. The object still EXISTS;
    // reporting it as "not found" sends the analyst chasing the wrong cause. It
    // must name the real reason: the object is stored in a packfile.
    let tr = TestRepo::new_two_commit();
    let root = tr.dir.path();
    let blob_hex = git_out(root, &["rev-parse", "HEAD:world.txt"]);
    git_out(root, &["repack", "-a", "-d", "-q"]); // pack everything, drop loose

    let repo = tr.repo();
    let hash = GitHash::from_hex(&blob_hex).expect("valid hash");
    let err = repo.read_object(&hash).expect_err("packed object is not loose");
    let msg = err.to_string();
    assert!(
        msg.contains("pack"),
        "a packed object must report packfile storage, not a misleading \
         not-found: got {msg:?}"
    );
}

// ── GitRepo::open ─────────────────────────────────────────────────────────────

#[test]
fn open_worktree_root() {
    let fix = TestRepo::new_two_commit();
    let _ = fix.repo(); // must not panic or return Err
}

#[test]
fn open_bare_git_dir() {
    let fix = TestRepo::new_two_commit();
    let git_dir = fix.dir.path().join(".git");
    let _ = GitRepo::open(&git_dir).expect("open bare .git dir");
}

#[test]
fn open_non_repo_returns_err() {
    let dir = tempfile::tempdir().unwrap();
    let result = GitRepo::open(dir.path());
    assert!(result.is_err(), "opening non-repo must return Err");
}

// ── HEAD resolution ───────────────────────────────────────────────────────────

#[test]
fn head_returns_a_hash() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD must resolve");
    assert_eq!(head.to_hex().len(), 40, "HEAD hash must be 40 hex chars");
}

#[test]
fn resolve_ref_main_equals_head() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let main = repo.resolve_ref("refs/heads/main").expect("refs/heads/main");
    assert_eq!(head, main, "HEAD and refs/heads/main must point to same commit");
}

// ── read_commit ───────────────────────────────────────────────────────────────

#[test]
fn read_head_commit() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let commit = repo.read_commit(&head).expect("read HEAD commit");
    assert_eq!(commit.hash, head);
    assert_eq!(commit.message.trim(), "second commit");
}

#[test]
fn commit_has_author_and_committer() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let commit = repo.read_commit(&head).expect("read commit");
    assert_eq!(commit.author.name, "Forensic Tester");
    assert_eq!(commit.author.email, "tester@forensic.example");
    assert!(commit.author.timestamp > 0, "author timestamp must be positive");
    assert_eq!(commit.committer.name, "Forensic Tester");
}

#[test]
fn commit_has_one_parent() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let commit = repo.read_commit(&head).expect("read commit");
    assert_eq!(commit.parents.len(), 1, "second commit must have one parent");
}

#[test]
fn root_commit_has_no_parents() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let tip = repo.read_commit(&head).expect("tip");
    let root = repo.read_commit(&tip.parents[0]).expect("root");
    assert_eq!(root.parents.len(), 0, "initial commit must have no parents");
    assert_eq!(root.message.trim(), "initial commit");
}

// ── read_tree ─────────────────────────────────────────────────────────────────

#[test]
fn read_tree_from_commit() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let commit = repo.read_commit(&head).expect("commit");
    let tree = repo.read_tree(&commit.tree).expect("read tree");
    assert!(!tree.entries.is_empty(), "tree must have entries");
    let names: Vec<&str> = tree.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"hello.txt"), "tree must contain hello.txt");
    assert!(names.contains(&"world.txt"), "tree must contain world.txt");
}

// ── read_blob ─────────────────────────────────────────────────────────────────

#[test]
fn read_blob_content() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let commit = repo.read_commit(&head).expect("commit");
    let tree = repo.read_tree(&commit.tree).expect("tree");
    let hello_entry = tree
        .entries
        .iter()
        .find(|e| e.name == "hello.txt")
        .expect("hello.txt must be in tree");
    let blob = repo.read_blob(&hello_entry.hash).expect("read blob");
    assert_eq!(blob, b"hello updated\n");
}

#[test]
fn blob_at_root_commit_has_original_content() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let tip = repo.read_commit(&head).expect("tip");
    let root = repo.read_commit(&tip.parents[0]).expect("root");
    let root_tree = repo.read_tree(&root.tree).expect("root tree");
    let hello_entry = root_tree
        .entries
        .iter()
        .find(|e| e.name == "hello.txt")
        .expect("hello.txt in root tree");
    let blob = repo.read_blob(&hello_entry.hash).expect("root blob");
    assert_eq!(blob, b"hello forensics\n");
}

// ── read_object / verification ────────────────────────────────────────────────

#[test]
fn raw_object_is_verified() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let obj = repo.read_object(&head).expect("read object");
    assert!(obj.verified, "SHA1 of object must verify against its hash");
    assert_eq!(obj.kind, ObjectKind::Commit);
}

#[test]
fn read_object_not_found_returns_err() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let fake = GitHash::from_hex("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef")
        .expect("valid hex");
    let result = repo.read_object(&fake);
    assert!(result.is_err(), "non-existent object must return Err");
}

// ── walk_commits ──────────────────────────────────────────────────────────────

#[test]
fn walk_commits_yields_newest_first() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let commits: Vec<_> = repo
        .walk_commits(head)
        .map(|r| r.expect("commit"))
        .collect();
    assert_eq!(commits.len(), 2, "must yield exactly 2 commits");
    assert_eq!(commits[0].message.trim(), "second commit");
    assert_eq!(commits[1].message.trim(), "initial commit");
}

#[test]
fn walk_commits_all_verified() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    for commit in repo.walk_commits(head) {
        let c = commit.expect("commit");
        let obj = repo.read_object(&c.hash).expect("read object");
        assert!(obj.verified, "commit {} must verify", c.hash);
    }
}

// ── GitHash ───────────────────────────────────────────────────────────────────

#[test]
fn hash_from_hex_roundtrip() {
    let hex = "da39a3ee5e6b4b0d3255bfef95601890afd80709";
    let h = GitHash::from_hex(hex).expect("valid hex");
    assert_eq!(h.to_hex(), hex);
}

#[test]
fn hash_from_hex_bad_length_returns_err() {
    assert!(GitHash::from_hex("deadbeef").is_err());
}

#[test]
fn hash_from_hex_bad_chars_returns_err() {
    assert!(GitHash::from_hex("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz").is_err());
}

// ── error cases ───────────────────────────────────────────────────────────────

#[test]
fn read_commit_on_blob_hash_returns_err() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    let head = repo.head().expect("HEAD");
    let commit = repo.read_commit(&head).expect("commit");
    let tree = repo.read_tree(&commit.tree).expect("tree");
    let blob_hash = tree.entries[0].hash;
    // read_commit on a blob hash must return Err, not panic.
    assert!(
        repo.read_commit(&blob_hash).is_err(),
        "read_commit on a blob hash must return Err"
    );
}

#[test]
fn resolve_nonexistent_ref_returns_err() {
    let fix = TestRepo::new_two_commit();
    let repo = fix.repo();
    assert!(
        repo.resolve_ref("refs/heads/no-such-branch").is_err(),
        "non-existent ref must return Err"
    );
}
