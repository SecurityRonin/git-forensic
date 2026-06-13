# git-forensic

[![git-core](https://img.shields.io/crates/v/git-core.svg?label=git-core)](https://crates.io/crates/git-core)
[![git-forensic](https://img.shields.io/crates/v/git-forensic.svg?label=git-forensic)](https://crates.io/crates/git-forensic)
[![Docs.rs](https://img.shields.io/docsrs/git-forensic)](https://docs.rs/git-forensic)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![CI](https://github.com/SecurityRonin/git-forensic/actions/workflows/ci.yml/badge.svg)](https://github.com/SecurityRonin/git-forensic/actions)
[![Sponsor](https://img.shields.io/badge/sponsor-h4x0r-ea4aaa?logo=github-sponsors)](https://github.com/sponsors/h4x0r)

**A from-scratch Git object-store reader and a graded anomaly auditor — read loose and packed objects straight off disk and surface the backdated commits, rewritten history, unsigned commits, and resurrectable dropped objects that a clean `git log` is built to hide.**

Two crates, one workspace:

- **[`git-core`](https://crates.io/crates/git-core)** — the reader: loose + packfile (v2, OFS/REF delta) objects, refs, commits, trees, and reflogs over a content-addressed Merkle DAG. Pure Rust, no `unsafe`, no `libgit2`/C bindings — reads any OS's `.git`.
- **[`git-forensic`](https://crates.io/crates/git-forensic)** — the auditor: turns parsed commits, reflogs, signatures, and reachability into severity-graded [`forensicnomicon::report::Finding`](https://crates.io/crates/forensicnomicon)s, so a repository's anomalies aggregate uniformly with the rest of the forensic fleet.

## Audit a repository in 30 seconds

```toml
[dependencies]
git-forensic = "0.1"   # pulls in git-core
```

```rust
use git_forensic::{audit_repo, source};
use git_core::GitRepo;
use std::path::Path;

let repo = GitRepo::open(Path::new("/path/to/.git"))?;
let head = repo.head()?;

// Walk every commit reachable from HEAD; get back graded anomalies.
for anomaly in audit_repo(&repo, head)? {
    let finding = anomaly.to_finding(source("repo"));
    println!("[{:?}] {} — {}", finding.severity, finding.code, finding.note);
    // e.g. [Some(Medium)] GIT-COMMIT-TIME-INVERSION — committer timestamp … precedes author …
}
# Ok::<(), git_core::GitError>(())
```

`audit_repo` walks commits from a starting hash and grades what it finds. Damaged or unreadable objects are surfaced as typed errors, never a panic.

## The anomaly codes

Each anomaly is an **observation** ("consistent with …"); the examiner draws the conclusions. Codes are a stable, published contract.

| Code | Severity | What it observes |
|---|---|---|
| `GIT-COMMIT-TIME-INVERSION` | Medium | A commit whose committer timestamp precedes its author timestamp — consistent with timestamp backdating (benign cause: cross-machine clock skew) |
| `GIT-HISTORY-REWRITE` | Medium | A reflog entry whose operation rewrote history (`reset` / `rebase` / `amend` / `filter-branch` / forced update) — the prior tip remains resurrectable |
| `GIT-UNSIGNED-IN-SIGNED-HISTORY` | Medium | An unsigned commit within a predominantly-signed history — consistent with a commit injected outside the prevailing signing policy (benign cause: a forgotten signature) |
| `GIT-UNREACHABLE-OBJECT` | Medium / Low | An object reachable from no ref — residue of deleted or rewritten history, resurrectable until `gc` (a dropped **commit** grades Medium; a loose blob/tree grades Low) |

Findings come from the analyzer's entry points: `audit_repo` / `audit_commit` (`GIT-COMMIT-TIME-INVERSION`), `audit_reflog` / `audit_reflog_entries` (`GIT-HISTORY-REWRITE`), `audit_signatures` / `audit_signatures_repo` (`GIT-UNSIGNED-IN-SIGNED-HISTORY`), and `audit_unreachable` (`GIT-UNREACHABLE-OBJECT`). The `attribution` module builds the who-did-what-when timeline an examiner narrates on.

## The reader: navigate an object store

`GitRepo` (in `git-core`) reads objects, refs, commits, and trees directly from a `.git` directory — loose objects and v2 packfiles alike:

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

The bare crate name `git` on crates.io is taken, so this crate publishes as `git-core` and imports as `git_core`.

## What makes this different from a general-purpose Git library

Most Git libraries answer one question: "what does this repository contain?" This workspace answers the questions a digital forensics examiner actually needs:

| Capability | General-purpose Git library | this workspace |
|---|---|---|
| Loose object read + inflate | ✅ | ✅ |
| Packfile v2 read (OFS / REF delta resolution) | ✅ | ✅ |
| Commit / tree / ref / reflog parsing | ✅ | ✅ |
| First-parent commit walk | ✅ | ✅ |
| Commit-time-inversion (backdating) detection | — | ✅ |
| Reflog history-rewrite residue (`reset` / `rebase` / `amend` / force-push) | — | ✅ |
| Unsigned-in-signed-history detection | — | ✅ |
| Unreachable-object (dropped-history) enumeration | — | ✅ |
| Attribution timeline (author/committer, timezone) | — | ✅ |
| Severity-graded `report::Finding` output | — | ✅ |
| Pure Rust, no `libgit2` / C bindings | partial | ✅ |
| `#![forbid(unsafe_code)]` | — | ✅ |

## Trust, but verify

`git-forensic` is built for untrusted object stores from potentially compromised systems:

- **`#![forbid(unsafe_code)]`** across both crates — no C bindings, no `libgit2`, no FFI. It reads any OS's `.git`.
- **Panic-free on malicious input** — every length, offset, and delta instruction is validated against both the declared size and the actual buffer; the workspace denies `clippy::unwrap_used` and `clippy::expect_used` in production code.
- **Fuzzed** — four `cargo-fuzz` targets (`loose`, `commit`, `tree`, `delta`); a `fuzz.yml` CI workflow builds and smoke-runs each.
- **Validated on real artifacts** — the reader is exercised against real `.git` directories, with object inflation and packfile delta resolution cross-checked against `git` itself.

```bash
cargo test
cargo +nightly fuzz run delta   # requires nightly + cargo-fuzz
```

## Where this fits

`git-core` is the Git content-addressed-store foundation for the SecurityRonin forensic family. It sits in the GRAPH NAVIGATION layer — navigating a Merkle DAG by hash — and feeds graded findings into [`issen`](https://github.com/SecurityRonin/issen) for cross-artifact correlation. Related fleet crates in the content-addressed and supply-chain space:

| Crate | Role |
|---|---|
| [`forensicnomicon`](https://crates.io/crates/forensicnomicon) | **KNOWLEDGE** — the shared `report::Finding` model every analyzer emits |
| [`issen`](https://github.com/SecurityRonin/issen) | **Orchestrator** — wires every forensic path and correlates findings |
| [`ntfs-forensic`](https://github.com/SecurityRonin/ntfs-forensic) | NTFS filesystem reader + anomaly auditor |

---

[Privacy Policy](https://securityronin.github.io/git-forensic/privacy/) · [Terms of Service](https://securityronin.github.io/git-forensic/terms/) · © 2026 Security Ronin Ltd
