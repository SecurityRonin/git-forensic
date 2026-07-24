# git-forensic — Purpose & Scope

*Library-tier intent document (per the fleet PRD & ADR Standard,
`ronin-issen/CLAUDE.md`). These crates are **linked**, not run — there is no
binary an examiner invokes — so this is a concise Purpose & Scope, not a product
requirements document. Every current-state claim below is grounded in a
same-session read of `core/src`, `forensic/src`, `Cargo.toml`, and the git
history (2026-07-24). The load-bearing decisions live as ADRs under
[`docs/decisions/`](decisions/).*

## What it is

A from-scratch Git object-store reader and a graded anomaly auditor, in one
workspace of two library crates:

- **`git-core`** (`core/`) — the reader. Navigates the content-addressed Merkle
  DAG of a `.git` directory by hash: loose objects (zlib-inflated), packfile v2
  (OFS/REF delta resolution, SHA-1-verified), refs, `packed-refs`, reflogs,
  commits, and trees. Pure Rust, `#![forbid(unsafe_code)]`, no `libgit2`/C
  bindings — it reads a Windows-, macOS-, or Linux-authored `.git` identically.
  Emits no findings.
- **`git-forensic`** (`forensic/`) — the analyzer. Turns parsed commits, reflogs,
  signatures, and reachability into severity-graded
  `forensicnomicon::report::Finding`s, so a repository's anomalies aggregate
  uniformly with the rest of the forensic fleet.

## Who links it

- **`issen`** (ORCHESTRATION) — consumes `git-forensic` findings for
  cross-artifact correlation and timeline building. In the fleet layer hierarchy
  `git-forensic`/`git-core` sit in **GRAPH NAVIGATION** (`[C]` content-addressed:
  `hash → blob → content graph`; identity *is* the hash).
- Any third-party Rust tool that needs to read a `.git` object store directly can
  depend on **`git-core`** alone, without compiling the finding model.

## What it does (the audits)

Each anomaly is an **observation** ("consistent with …"), never a legal
conclusion; the examiner draws the conclusion. Codes are a published contract:

| Code | Severity | What it observes |
|---|---|---|
| `GIT-COMMIT-TIME-INVERSION` | Medium | Committer timestamp precedes author timestamp — consistent with backdating (benign cause: clock skew) |
| `GIT-HISTORY-REWRITE` | Medium | A reflog entry recording a history-rewriting operation (`reset`/`rebase`/`amend`/force) — the prior tip stays resurrectable |
| `GIT-UNSIGNED-IN-SIGNED-HISTORY` | Medium | An unsigned commit within a predominantly-signed history |
| `GIT-UNREACHABLE-OBJECT` | Medium (commit) / Low (blob, tree) | An object reachable from no ref — residue of dropped/rewritten history, resurrectable until `gc` |

The `attribution` module builds the author/committer who-did-what-when timeline
(with timezone retention) that an examiner narrates on. Entry points:
`audit_repo`/`audit_commit`, `audit_reflog`/`audit_reflog_entries`,
`audit_signatures`/`audit_signatures_repo`, `audit_unreachable`.

## Scope

- Read loose + packfile-v2 objects, refs, `packed-refs`, reflogs, commits, and
  trees from an on-disk `.git`, OS-agnostically.
- Grade the four anomaly classes above into `forensicnomicon` findings.
- Treat the object store as untrusted evidence: panic-free on malformed input,
  fuzzed per parsed structure, fail-loud on bootstrap failure (ADR 0004, 0007).

## Non-goals

- **No mutation of the repository.** The reader is read-only; it never writes to
  the `.git`.
- **No general-purpose Git library surface.** No staging, no commit creation, no
  network/transfer protocol, no working-tree checkout — only forensic reading and
  auditing.
- **No runnable front-end.** No CLI, GUI, or MCP server ships here; the
  user-facing surface is `issen`. The `fuzz/` targets are development harnesses,
  not an examiner tool.
- **No packfile v3 / SHA-256 object format** at present — the reader targets
  packfile v2 and SHA-1 object names (`hash.rs`, `pack.rs`).
- **No C/FFI or `unsafe`** — pure-Rust dependencies only (ADR 0003).

## Validation approach

The reader is exercised against real `.git` directories produced by the actual
`git` binary (Doer-Checker): object inflation and packfile delta resolution are
cross-checked against `git cat-file`, and cross-platform (mac/linux/windows)
`.git` parsing is covered (commit `af55415`). Four `cargo-fuzz` targets (`loose`,
`commit`, `tree`, `delta`) assert the no-panic invariant on adversarial input, and
the analyzer decision cores are covered end-to-end (build a valid object → parse →
extract → audit).
