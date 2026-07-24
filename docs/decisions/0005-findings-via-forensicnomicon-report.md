# 5. Anomalies emit as `forensicnomicon::report` observations, not a bespoke type

Date: 2026-07-24
Status: Accepted

## Context

Every analyzer in the SecurityRonin fleet must aggregate uniformly so that
`issen` (ORCHESTRATION) and a future GUI render findings from all artifact
families through one model, rather than N bespoke `XxxAnalysis` types. The fleet's
reporting model (`ronin-issen/CLAUDE.md`, "The Reporting Model") makes
`forensicnomicon::report::Finding` — with `Severity`, `Category`, a published
`code`, `note`, `evidence`, and `Source` — that single vocabulary. Findings must
read as *observations* ("consistent with …"), never legal conclusions.

## Decision

`git-forensic` keeps its typed domain enums (`GitAnomaly`, `ReflogAnomaly`,
`SignatureAnomaly`, `UnreachableObject`) as its own knowledge, and converts each
to a canonical `Finding` by implementing `forensicnomicon::report::Observation`
(`forensic/src/lib.rs`, `impl Observation for GitAnomaly`, and likewise per
module). Concretely:

- Each anomaly returns a stable, scheme-prefixed SCREAMING-KEBAB `code` —
  `GIT-COMMIT-TIME-INVERSION`, `GIT-HISTORY-REWRITE`,
  `GIT-UNSIGNED-IN-SIGNED-HISTORY`, `GIT-UNREACHABLE-OBJECT` — treated as a
  published contract.
- Severity grading encodes forensic judgment: a commit-time inversion grades
  `Medium` (real irregularity, but a common benign cause — clock skew), and an
  unreachable *commit* grades `Medium` while an unreachable blob/tree grades `Low`.
- `note`s are phrased as "consistent with …" observations; a `source(scope)`
  helper stamps analyzer provenance (`analyzer: "git-forensic"`, crate version).
- `Category` reflects the analytical lens (`History` for backdating/rewrite,
  `Integrity` for signature gaps, `Residue` for unreachable objects).

## Consequences

- A Git repository's anomalies drop into the same `Report` aggregation as NTFS,
  EVTX, SRUM, browser, and memory findings, with no adapter in the orchestrator.
- New anomaly kinds get new codes; a shipped code is never repurposed.
- The analyzer owns the Git-specific severity/category mapping (the domain
  knowledge), while `forensicnomicon` stays the leaf and enumerates no Git anomaly
  kinds — matching the fleet's producer pattern.
