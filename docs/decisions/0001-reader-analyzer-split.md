# 1. Two-crate reader/analyzer split (git-core + git-forensic)

Date: 2026-07-24
Status: Accepted

## Context

The repository began as a single `git-forensic` crate that implemented `[C]`
content-addressed navigation — `hash → commit → tree → blob` over the Merkle DAG
(commit `8e07f62`, "feat(git-forensic): implement [C] content-addressed
navigation"). As the packfile reader and the anomaly auditor both grew, two
distinct responsibilities lived in one crate: reading raw Git objects, and
grading forensic anomalies. A third-party Rust tool that only wants to read a
`.git` object store would have been forced to compile the whole finding/severity
surface, and the reader and the auditor could not evolve or version
independently.

The fleet's Crate-structure standard (`ronin-issen/CLAUDE.md`) mandates a
reader/analyzer split for every format: a `<x>-core` reader that exposes raw
navigation with no findings, and a `<x>-forensic` analyzer that emits graded
findings.

## Decision

Split into one workspace with two members (commit `6ae11ce`, "refactor+RED:
split into git-core (reader) + git-forensic (analyzer)"; `Cargo.toml`
`members = ["core", "forensic"]`):

- **`core/` → `git-core`** — the reader. It navigates the content-addressed
  store: loose objects with zlib inflation, packfile v2 with OFS/REF delta
  resolution, refs, `packed-refs`, reflogs, commits, and trees, over a
  `.git` directory. It emits no findings.
- **`forensic/` → `git-forensic`** — the analyzer. It consumes `git-core`
  (`forensic/Cargo.toml`: `git-core = { version = "0.2", path = "../core" }`),
  walks parsed commits/reflogs/signatures/reachability, and emits severity-graded
  `forensicnomicon::report::Finding`s.

The dependency direction is one-way: `git-forensic → git-core → forensicnomicon`.
The analyzer holds its own typed `GitAnomaly` / `ReflogAnomaly` /
`SignatureAnomaly` / `UnreachableObject` domain enums and converts them to
canonical findings, rather than pushing anomaly knowledge down into the reader.

## Consequences

- A downstream tool that only needs to read a `.git` depends on `git-core` alone
  and never compiles the finding model.
- The reader and analyzer version and publish independently (both at 0.2.0 as of
  writing).
- The fleet's binding principle that a `-forensic` analyzer *may* drop below its
  `-core` reader when the reader's API hides an anomaly is available but not yet
  exercised here: the current audits (time inversion, reflog rewrite, signature
  gaps, unreachability) are all expressible over `git-core`'s public navigation,
  so `git-forensic` depends on `git-core` directly.
- The layering must stay acyclic; shared Git-object knowledge belongs in
  `git-core`, shared finding vocabulary in `forensicnomicon`.
