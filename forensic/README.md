# git-forensic

[![git-forensic](https://img.shields.io/crates/v/git-forensic.svg?label=git-forensic)](https://crates.io/crates/git-forensic)
[![git-core](https://img.shields.io/crates/v/git-core.svg?label=git-core)](https://crates.io/crates/git-core)
[![Docs.rs](https://img.shields.io/docsrs/git-forensic)](https://docs.rs/git-forensic)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE)
[![CI](https://github.com/SecurityRonin/git-forensic/actions/workflows/ci.yml/badge.svg)](https://github.com/SecurityRonin/git-forensic/actions)
[![Sponsor](https://img.shields.io/badge/sponsor-h4x0r-ea4aaa?logo=github-sponsors)](https://github.com/sponsors/h4x0r)

**Point it at a `.git`, get back severity-graded Git anomalies — backdated commits, rewritten history, unsigned commits in a signed history, and resurrectable dropped objects as `forensicnomicon::report::Finding`s.**

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

for anomaly in audit_repo(&repo, head)? {
    let finding = anomaly.to_finding(source("repo"));
    println!("[{:?}] {} — {}", finding.severity, finding.code, finding.note);
    // e.g. [Some(Medium)] GIT-COMMIT-TIME-INVERSION — committer timestamp … precedes author …
}
# Ok::<(), git_core::GitError>(())
```

`audit_repo` walks commits from a starting hash and grades what it finds. Damaged or unreadable objects surface as typed errors, never a panic.

## The anomaly codes

Each anomaly is an **observation** ("consistent with …"); the examiner draws the conclusions. Codes are a stable, published contract.

| Code | Severity | What it observes |
|---|---|---|
| `GIT-COMMIT-TIME-INVERSION` | Medium | A commit whose committer timestamp precedes its author timestamp — consistent with timestamp backdating (benign cause: cross-machine clock skew) |
| `GIT-HISTORY-REWRITE` | Medium | A reflog entry whose operation rewrote history (`reset` / `rebase` / `amend` / `filter-branch` / forced update) — the prior tip remains resurrectable |
| `GIT-UNSIGNED-IN-SIGNED-HISTORY` | Medium | An unsigned commit within a predominantly-signed history — consistent with a commit injected outside the prevailing signing policy (benign cause: a forgotten signature) |
| `GIT-UNREACHABLE-OBJECT` | Medium / Low | An object reachable from no ref — residue of deleted or rewritten history, resurrectable until `gc` (a dropped **commit** grades Medium; a loose blob/tree grades Low) |

`GIT-COMMIT-TIME-INVERSION` comes from `audit_commit` / `audit_repo`; `GIT-HISTORY-REWRITE` from `audit_reflog` / `audit_reflog_entries` (`classify_rewrite` is the pure decision core); `GIT-UNSIGNED-IN-SIGNED-HISTORY` from `audit_signatures` / `audit_signatures_repo`; `GIT-UNREACHABLE-OBJECT` from `audit_unreachable`. Each anomaly emits a graded `report::Finding` via `to_finding(source)`.

## The attribution timeline

Beyond the graded anomalies, the `attribution` module flattens a set of commits into the who-did-what-when backbone an examiner narrates on:

- `attribution_timeline(commits)` → a time-ordered stream of author/committer identity events, timezone offset retained (it can corroborate or contradict a claimed location).
- `distinct_identities(commits)` → the distinct `(name, email)` identities, in first-seen order.
- `attribution_repo(repo, from)` → the same, walked from a repository tip.

## The two-crate split

This crate is the **analyzer**; the **reader** is [`git-core`](https://crates.io/crates/git-core) (loose + packfile objects, refs, commits, trees, reflogs, and `GitRepo` navigation over a `.git` directory). The split mirrors `ntfs-core`/`ntfs-forensic`. Together they feed [`issen`](https://github.com/SecurityRonin/issen) for cross-artifact correlation.

## Trust, but verify

Built for untrusted object stores from potentially compromised systems: `#![forbid(unsafe_code)]`; panic-free on crafted input (the workspace denies `clippy::unwrap_used` / `expect_used` in production code); `git-core` is fuzzed with four `cargo-fuzz` targets and exercised against real `.git` directories with packfile delta resolution cross-checked against `git` itself.

---

[Privacy Policy](https://securityronin.github.io/git-forensic/privacy/) · [Terms of Service](https://securityronin.github.io/git-forensic/terms/) · © 2026 Security Ronin Ltd
