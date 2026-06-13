# git-core

[![git-core](https://img.shields.io/crates/v/git-core.svg?label=git-core)](https://crates.io/crates/git-core)
[![git-forensic](https://img.shields.io/crates/v/git-forensic.svg?label=git-forensic)](https://crates.io/crates/git-forensic)
[![Docs.rs](https://img.shields.io/docsrs/git-core)](https://docs.rs/git-core)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![CI](https://github.com/SecurityRonin/git-forensic/actions/workflows/ci.yml/badge.svg)](https://github.com/SecurityRonin/git-forensic/actions)
[![Sponsor](https://img.shields.io/badge/sponsor-h4x0r-ea4aaa?logo=github-sponsors)](https://github.com/sponsors/h4x0r)

**A from-scratch, forensic-grade Git object-store reader — loose + packfile (v2, OFS/REF delta) objects, refs, commits, trees, and reflogs over a content-addressed Merkle DAG. No `unsafe`, no `libgit2`, no C bindings — reads any OS's `.git`.**

```toml
[dependencies]
git-core = "0.1"
```

```rust
use git_core::GitRepo;
use std::path::Path;

let repo = GitRepo::open(Path::new("/path/to/.git"))?;

// Resolve HEAD and read the commit it points at…
let head = repo.head()?;
let commit = repo.read_commit(&head)?;
println!("{} by {}", commit.hash.to_hex(), commit.author.name);

// …walk first-parent history, or enumerate every object in the store.
for commit in repo.walk_commits(head) {
    let _ = commit?;
}
for hash in repo.all_objects()? {
    println!("{}", hash.to_hex());
}
# Ok::<(), git_core::GitError>(())
```

The bare crate name `git` on crates.io is taken, so this crate publishes as **`git-core`** and imports as **`git_core`**.

## What it parses

`GitRepo` (`open`, `head`, `resolve_ref`, `read_object`, `read_commit`, `read_tree`, `read_blob`, `walk_commits`, `reflog`, `all_objects`, `all_refs`) · loose object read + zlib inflation (`object`, `loose`) · packfile v2 read with OFS and REF delta resolution (`pack`) · commit parsing with author/committer signatures, parents, tree, message, and `gpgsig` presence (`commit::CommitObject`, `Signature`) · tree parsing (`tree::TreeObject`, `TreeEntry`) · ref + reflog parsing (`refs`, `reflog::parse_reflog`, `ReflogEntry`) · `GitHash` (SHA-1) with hex/byte conversion and object-path derivation.

## Trust, but verify

`#![forbid(unsafe_code)]`; panic-free on crafted input (the workspace denies `clippy::unwrap_used` / `expect_used` in production code, every length, offset, and delta instruction bounds-checked); fuzzed with four `cargo-fuzz` targets (`loose`, `commit`, `tree`, `delta`); the reader is exercised against real `.git` directories with object inflation and packfile delta resolution cross-checked against `git` itself.

## Forensic analysis

Severity-graded anomaly auditing (commit-time inversion / history-rewrite residue / unsigned-in-signed-history / unreachable-object findings) lives in the sibling **[`git-forensic`](https://crates.io/crates/git-forensic)** crate, built on this one — the reader/analyzer split mirrors `ntfs-core`/`ntfs-forensic`.

---

[Privacy Policy](https://securityronin.github.io/git-forensic/privacy/) · [Terms of Service](https://securityronin.github.io/git-forensic/terms/) · © 2026 Security Ronin Ltd
