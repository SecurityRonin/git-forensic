# 3. From-scratch pure-Rust object-store reader; `forbid(unsafe)`, no C bindings

Date: 2026-07-24
Status: Accepted

## Context

Reading a `.git` object store can be delegated to `libgit2` (C, via FFI bindings)
or to an existing pure-Rust implementation (`gitoxide`/`gix`). But this reader
parses **untrusted, attacker-controllable** object stores lifted from
potentially-compromised systems, and it is a *forensic* reader: it must see what
a happy-path Git library abstracts away — unreachable objects, raw packfile
structure, damaged/malformed objects, and residue of dropped history. The fleet's
binding design principle (`ronin-issen/CLAUDE.md`, Crate-structure standard) is
that a forensic reader "often needs to go much lower level than the `-core` API"
of a robust reader that is built to read *valid* data. A C-FFI dependency also
carries the memory-corruption/RCE liability that the fleet's `unsafe` law treats
as the worst kind of `unsafe` — the compiler has zero visibility into C.

## Decision

Implement the reader from scratch in pure Rust with only vetted pure-Rust
dependencies and `#![forbid(unsafe_code)]` across the whole workspace:

- Dependencies are `flate2` (zlib inflation), `sha1` (SHA-1 verification), `hex`,
  and `thiserror` (`core/Cargo.toml`) — no `libgit2`, no `-sys` crate, no FFI.
- `unsafe_code = "forbid"` is set at the workspace level
  (`Cargo.toml` `[workspace.lints.rust]`), and `git-forensic` also carries an
  explicit `#![forbid(unsafe_code)]` (`forensic/src/lib.rs`). Unlike the fleet's
  mmap readers (`ewf`, `memory-forensic`), which downgrade to
  `unsafe_code = "deny"` + a bounded per-site allow, this reader needs no `mmap`
  and no `unsafe` at all, so full `forbid` is achievable and kept.
- The reader owns the low-level structure: `pack.rs` walks the `.idx` v2 layout,
  resolves OFS/REF deltas recursively, and verifies the SHA-1 of every
  reconstructed object (`verify(...)` in `pack.rs`).

## Consequences

- The reader compiles anywhere Rust does, with no C toolchain, and can wear the
  `unsafe forbidden` guarantee (no crafted input can reach a raw-pointer path).
- Reimplementing a format parser carries the offset/bit-split/endianness risk the
  fleet guards against; this is mitigated by fuzzing and real-artifact validation
  against the `git` binary (ADR 0004).
- The reader is free to expose forensic-relevant structure (object enumeration,
  reachability) that a general-purpose library would not, feeding the analyzer's
  unreachable-object and history-rewrite audits.

## Note on recovered rationale

The *decision* (from-scratch pure-Rust reader, `forbid(unsafe)`, no C bindings)
is directly visible in the code and dependency tree, and its rationale is
grounded in the fleet `unsafe` law and the forensic-lower-level principle. A
head-to-head evaluation of `gitoxide`/`libgit2` as rejected alternatives is
**not** recorded in the available git history; that specific comparison is
reconstructed from fleet principles, not from a documented original deliberation.
