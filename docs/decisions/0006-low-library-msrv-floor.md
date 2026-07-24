# 6. Low, CI-verifiable library MSRV floor (1.81), decoupled from the dev toolchain

Date: 2026-07-24
Status: Accepted

## Context

Both crates in this workspace are **libraries** — `git-core` is a reader linked by
other tools, and `git-forensic` is an analyzer linked into `issen` and downstream
consumers. Neither ships a binary an examiner runs. The fleet MSRV policy
(`ronin-issen/CLAUDE.md`, "Rust MSRV & Toolchain") separates the *dev toolchain*
(what the fleet builds/fmt/clippy with) from the *declared MSRV* (a
downstream-facing compatibility promise). Published libraries keep a low,
CI-verified MSRV; only apps declare MSRV equal to the pinned toolchain. Raising a
published library's MSRV narrows its crates.io audience and is treated as a
near-breaking change.

## Decision

Declare a low library MSRV of **1.81** at the workspace level
(`Cargo.toml` `[workspace.package]` `rust-version = "1.81"`), inherited by both
members. Pin the *dev* toolchain separately to the current fleet stable
(`rust-toolchain.toml` `channel = "1.96.0"`, with `rustfmt` + `clippy`
components — commit `54cf2e1`). The two numbers are deliberately different: the
pinned 1.96.0 is what contributors and CI build with; the declared 1.81 is the
oldest toolchain the libraries promise to compile on.

## Consequences

- A third-party consumer on an older toolchain (down to 1.81) can link `git-core`
  or `git-forensic`.
- The library forgoes any Rust feature newer than 1.81 workspace-wide; raising the
  floor requires a genuine need and is a deliberate, near-breaking bump.
- Development and CI churn is eliminated by the single pinned dev toolchain, while
  the low declared floor stays a real, CI-verifiable guarantee rather than
  drifting with the pin.
