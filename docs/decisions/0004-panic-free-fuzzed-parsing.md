# 4. Panic-free, fuzzed parsing of untrusted object stores (Paranoid Gatekeeper)

Date: 2026-07-24
Status: Accepted

## Context

Every byte this workspace parses is attacker-controllable: a loose object header,
a packfile varint length, a delta copy/insert instruction, a reflog line. A
length field that lies, a truncated object, a delta base that cycles, or a
zlib-bomb blob must never crash the tool or, worse, produce silently wrong
output. A forensic reader that panics on a crafted `.git` is a denial-of-service
on the investigation. This is the fleet's Paranoid Gatekeeper standard applied to
the Git object store (commit `3961569`, "chore(security): Paranoid Gatekeeper
bringup for git-core + git-forensic").

## Decision

Enforce a panic-free posture statically and dynamically:

- **Static lints** (`Cargo.toml` `[workspace.lints.clippy]`):
  `unwrap_used = "deny"`, `expect_used = "deny"`, `correctness`/`suspicious`
  denied, `all`/`pedantic` warned. Tests opt out via
  `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]`.
- **Bounded reads and caps** in the reader: `pack.rs` rejects any object
  decompressing past `MAX_OBJECT_SIZE` (512 MiB, deflate-bomb guard) and any
  delta chain deeper than `MAX_DELTA_DEPTH` (64, cycle guard); every length,
  offset, and delta instruction is validated against both the declared size and
  the actual buffer before use. Damaged or unreadable objects surface as typed
  `GitError`s, never a panic.
- **Fuzzing** — four `cargo-fuzz` targets under `fuzz/` (`loose`, `commit`,
  `tree`, `delta`), one per parsed structure, with the invariant that no input
  may panic. A `fuzz.yml` CI workflow builds and smoke-runs each; the targets run
  on nightly (commit `68fa88b`, "fix(ci): run cargo-fuzz on nightly").

## Consequences

- Malformed evidence degrades to a typed error or a verified-`false` object,
  never a crash or a raw-pointer path.
- The static lints require more verbose, explicitly-bounds-checked code than a
  quick `unwrap` would; that verbosity is the guarantee.
- The four fuzz targets are part of the maintained surface and run in CI; a new
  parsed structure is expected to add its own target.
- The README's robustness claim leads with the *measured* evidence
  ("Fuzzed — four `cargo-fuzz` targets") beside the qualified static half
  ("panic-free on malicious input … denies `unwrap_used`/`expect_used`"), per the
  fleet's Evidence-Based Rigor wording rule.
