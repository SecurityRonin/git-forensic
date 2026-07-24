# 2. Reader publishes as `git-core`, imports as `git_core`

Date: 2026-07-24
Status: Accepted

## Context

The reader crate would naturally be named `git` on crates.io. That bare name is
taken by a popular, unrelated third-party crate. The fleet naming grammar
(`ronin-issen/CLAUDE.md`, "Crate naming grammar") distinguishes two cases: when
the bare `<x>` name is held by an *obscure* crate we can co-exist with, publish
`<x>-core` with `[lib] name = "<x>"` so consumers write `use <x>::…`; but when
the bare name is a *popular* crate, do **not** hijack the import path — keep the
`<x>_core` import (the reference case in the constitution is `ntfs` = Colin
Finck's crate, so `ntfs-core` imports as `ntfs_core`).

## Decision

Publish the reader as **`git-core`** and let it import as **`git_core`** — the
default lib name derived from the package name, with no `[lib] name` override
(`core/Cargo.toml`: `name = "git-core"`, no `[lib]` stanza). The analyzer stays
**`git-forensic`**. The README states this explicitly: "The bare crate name `git`
on crates.io is taken, so this crate publishes as `git-core` and imports as
`git_core`."

## Consequences

- No collision with, and no attempt to co-opt, the popular `git` crate's
  namespace; the `git_core` import path is unambiguous about what it is.
- Consumers write `use git_core::GitRepo;` and `use git_forensic::audit_repo;` —
  the two-crate identity is visible at the call site.
- Follows the fleet reader = `<x>-core` / analyzer = `<x>-forensic` convention,
  so the pair reads consistently alongside `ntfs-core`/`ntfs-forensic`,
  `vmdk-core`/`vmdk-forensic`, etc.
