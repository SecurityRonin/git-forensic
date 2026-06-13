# Changelog

All notable changes to `git-forensic` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [git-core 0.1.0 / git-forensic 0.1.0] — 2026-06-13

Initial crates.io release.

### Added — `git-core` (reader)

- From-scratch Git object-store reader over a `.git` directory — no `libgit2`, no
  C bindings, pure Rust.
- Loose object read + zlib inflation (`object`, `loose`).
- Packfile v2 read with OFS and REF delta resolution (`pack`).
- Commit parsing — author/committer signatures, parents, tree, message, and
  `gpgsig` presence (`commit::CommitObject`, `Signature`).
- Tree parsing (`tree::TreeObject`, `TreeEntry`).
- Ref and reflog parsing (`refs`, `reflog::parse_reflog`, `ReflogEntry`).
- `GitHash` (SHA-1) with hex/byte conversion and object-path derivation.
- `GitRepo` navigation: `open`, `head`, `resolve_ref`, `read_object`,
  `read_commit`, `read_tree`, `read_blob`, `walk_commits`, `reflog`,
  `all_objects`, `all_refs`.

### Added — `git-forensic` (analyzer)

- `GIT-COMMIT-TIME-INVERSION` (Medium / History) — a commit whose committer
  timestamp precedes its author timestamp (`audit_commit`, `audit_repo`).
- `GIT-HISTORY-REWRITE` (Medium / History) — reflog entries recording a
  history-rewriting operation (`audit_reflog`, `audit_reflog_entries`,
  `classify_rewrite`).
- `GIT-UNSIGNED-IN-SIGNED-HISTORY` (Medium / Integrity) — an unsigned commit
  within a predominantly-signed history (`audit_signatures`,
  `audit_signatures_repo`).
- `GIT-UNREACHABLE-OBJECT` (Medium for commits, Low for blobs/trees / Residue) —
  objects reachable from no ref (`audit_unreachable`).
- `attribution` — author/committer attribution timeline with timezone retention
  (`attribution_timeline`, `distinct_identities`, `attribution_repo`).
- Each anomaly emits a graded `forensicnomicon::report::Finding` via the
  `Observation` trait; `source(scope)` stamps the analyzer provenance.

### Security

- `#![forbid(unsafe_code)]` across both crates.
- Adversarial-input hardening: bounded reads, delta-instruction validation, and
  typed errors rather than panics or silently-wrong output.
- Four `cargo-fuzz` targets (`loose`, `commit`, `tree`, `delta`).

### Testing

- 100% line coverage of the analyzer decision cores.
- Reader exercised against real `.git` directories with object inflation and
  packfile delta resolution.

[Unreleased]: https://github.com/SecurityRonin/git-forensic/compare/v0.1.0...HEAD
[git-core 0.1.0 / git-forensic 0.1.0]: https://github.com/SecurityRonin/git-forensic/releases/tag/v0.1.0
