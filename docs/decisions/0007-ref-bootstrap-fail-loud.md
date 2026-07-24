# 7. Ref-bootstrap failure fails loud, never inverts to all-unreachable

Date: 2026-07-24
Status: Accepted

## Context

`audit_unreachable` computes `all_objects − reachable`, where the reachable set is
seeded from every ref tip (`reachable_set` in `forensic/src/unreachable.rs`). The
original implementation seeded its roots from `all_refs()`, whose enumeration
**silently swallowed every I/O error** — a failed `read_dir`/`read` on `refs/`,
`packed-refs`, or `HEAD` collapsed to an empty root set. With zero roots, *every*
object in the store was flagged `GIT-UNREACHABLE-OBJECT`. That false-positive
inversion is indistinguishable from a genuinely all-orphaned repository — the
canonical "bootstrap failure masquerading as an empty/normal result" bug the
fleet robustness law (`ronin-issen/CLAUDE.md`, "Robustness — Bootstrap failure ≠
artifact-not-found") exists to prevent (commit `850a000` RED test, `7aeb5f1` fix).

## Decision

Separate a *bootstrap* failure (the whole refs subsystem is unreadable) from a
*per-artifact* miss (one ref won't resolve), and surface the former loudly:

- `list_refs_checked` / `collect_loose_refs_checked` propagate any non-`NotFound`
  `read_dir`/`read` error as `GitError::Io`. An absent `refs/`, `packed-refs`, or
  `HEAD` stays the legitimate empty case; an individual unresolvable ref
  (`RefNotFound`) is still skipped as a per-artifact miss.
- `GitRepo::all_refs_checked` exposes the fail-loud path; `list_refs` remains a
  best-effort `unwrap_or_default` wrapper for callers that genuinely want lenient
  enumeration.
- `reachable_set` returns `Result` and `audit_unreachable` propagates via `?`, so
  a ref-bootstrap failure errors rather than emitting a store-wide finding. A
  regression test (`audit_unreachable_errs_when_ref_bootstrap_fails`) asserts a
  failed bootstrap errors instead of reporting all objects unreachable.

## Consequences

- A corrupt or unreadable refs subsystem produces a diagnostic error, not a
  fabricated wall of unreachable-object findings.
- The empty-but-valid case (a legitimately ref-less store) and the failed-bootstrap
  case are now distinguishable — the former is empty, the latter errors.
- The reader gained a `_checked` variant beside the lenient one, keeping both the
  fail-loud and best-effort contracts explicit at the call site.
