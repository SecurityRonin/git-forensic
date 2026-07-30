# 8. A `[C]` navigator, not a `forensic-vfs` filesystem

Date: 2026-07-30
Status: Accepted

## Context

Every disk/container/filesystem reader in the fleet implements the `forensic-vfs`
contracts, and the fleet rule (`ronin-issen/docs/decisions/0011-vfs-universal-container-abstraction.md`)
is that a consumer reading an evidence image must depend on that abstraction and
never on a per-format crate. `git-forensic` does not implement those contracts,
which invites the question of whether it is a gap in the VFS coverage.

**What the code shows (verified this session):**

- Neither crate depends on `forensic-vfs`. `core/Cargo.toml` declares only
  `flate2`, `thiserror`, `hex`, `sha1`; `forensic/Cargo.toml` declares `git-core`
  and `forensicnomicon`. The workspace `Cargo.lock` contains no `vfs` entry at
  all.
- The reader's whole navigation surface is hash-keyed. `core/src/repo.rs` exposes
  `read_object(&GitHash)`, `read_commit`, `read_tree`, `read_blob`,
  `walk_commits(GitHash)`, `all_objects() -> Vec<GitHash>`, `all_refs_checked()`,
  and `reflog(refname)`. The only name-bearing structure in the reader is
  `TreeEntry { mode, name, hash }` (`core/src/tree.rs`) — a name-to-hash edge
  *inside one immutable snapshot object*, not a mutable directory a filesystem
  could be mounted over.
- The analyzer (`forensic/src/lib.rs`: `attribution`, `reflog`, `signatures`,
  `unreachable`, plus `audit_repo`/`audit_commit`/`source`) consumes that
  hash-keyed surface and emits `forensicnomicon::report::Finding`s. Nothing in it
  addresses a byte offset on an image.
- `forensic_vfs::Layer` (`forensic-vfs` 0.7.0, published to crates.io;
  `crates/core/src/locator.rs:47`) has nine variants — `File`, `Range`,
  `Container`, `Volume`, `Encryption`, `Snapshot`, `Fs`, `Stream`, `Archive`.
  **There is no content-addressed variant.**
- `forensic_vfs::FileSystem` (`crates/core/src/fs.rs:341`) is a `[P]` interface
  end to end: `root() -> FileId`, `lookup(parent, name)`, `read_dir(ino)`,
  `meta(ino) -> FsMeta`, `extents(ino, stream)`, `read_at(ino, stream, off, buf)`,
  plus `deleted()`, `unallocated()`, and `slack(ino, stream)`.
- The resolver's terminal types are hard-typed to that trait:
  `Resolved { fs: forensic_vfs::DynFs, .. }` and `Evidence { fs: Option<DynFs>, .. }`
  (`crates/resolver/src/lib.rs:53-72`).

The fleet architecture already separates these two things.
`ronin-issen/docs/decisions/0016-multi-repo-layer-architecture.md` defines FILESYSTEM
as `[P]` "navigate a sector stream by path (name -> inode -> block)" and GRAPH NAV
as `[C]` "navigate a content-addressed store by hash (Merkle DAG)", and it lists
`git-forensic` under GRAPH NAV. `docs/PRD.md` in this repo states the same
placement.

## Decision

**`git-forensic`/`git-core` are reached directly as a `[C]` content-addressed
navigator. They are not wired into `forensic-vfs`, and their absence from it is
not a VFS gap.**

`forensic_vfs::FileSystem` models `[P]`: a hierarchical name-to-inode-to-block
namespace, where identity is a `FileId` slot that is allocated, reallocated, and
freed, and where the interesting forensic residue lives in the gaps
(`deleted()`, `unallocated()`, `slack()`). A Git object store is `[C]`: a
hash-keyed DAG where identity *is* the content hash, nothing is ever reallocated,
and objects are immutable.

Forcing the store into `FileSystem` means inventing a synthetic namespace over
content-addressed storage, and every part of that invention states something
untrue about the evidence:

- **The namespace does not exist in the data.** A blob has a path only relative
  to one chosen commit's tree, and it has N different paths across M commits. A
  `lookup(parent, name)` surface asserts a single directory that was never
  recorded. Worse, the objects this analyzer exists to surface —
  `forensic/src/unreachable.rs`, `GIT-UNREACHABLE-OBJECT` — are reachable from no
  ref, so they have *no* path under any synthetic scheme.
- **`deleted()` would misdescribe them.** Unreachable objects are intact,
  present, and resurrectable until `gc`; they are not deleted directory entries.
  Reporting them through a "deleted node" surface converts an accurate
  observation into a misleading one.
- **`unallocated()` and `slack()` have no referent.** Git has no block allocator,
  so any value returned would be fabricated.
- **`extents()` cannot be answered honestly.** A loose object is zlib-deflated
  (`core/src/loose.rs`) and a packed object is delta-encoded against another
  object through an OFS/REF chain (`core/src/pack.rs`), so a logical object has no
  contiguous byte run on the image to cite. An extent emitted for one would be a
  false location in a report.
- **`FsMeta`/`MacbTimes` are the wrong metadata.** A Git object carries no MACB
  times; author and committer timestamps are *content* inside the commit object
  (`core/src/commit.rs`), which is precisely why they can be backdated and why
  `GIT-COMMIT-TIME-INVERSION` is a finding at all.

That misrepresentation is the failure ADR-0011's abstraction rule exists to
prevent. The rule's target is a consumer that special-cases one *image* format
instead of asking the abstraction; it is not a mandate to route a non-`[P]`
primitive through a `[P]` interface. ADR-0011 already makes this distinction
itself, refusing to "shoehorn a file archive into the raw-disk contract" and
giving logical containers their own typed path.

**The apparent counter-example resolves the same way.** `forensic-vfs` ADR-0014
does surface zip/7z/tar and AD1/AFF4-Logical containers as browsable
`FileSystem`s — but on the explicit condition that the filesystem "exposes files
at their NATURAL paths", i.e. the container's own member table already *is* a
name-to-bytes tree. A Git object store has no such member table; its natural
addressing is the hash.

**How the two compose today, with no VFS change.** `GitRepo::open` takes a
`&Path` (`core/src/repo.rs`) and reads through `std::fs` (`core/src/loose.rs`,
`pack.rs`, `refs.rs`, `reflog.rs`). A `.git` inside an evidence image is therefore
reached by the consumer mounting the image through the VFS (`forensic-vfs` /
`4n6mount`) and handing `git-core` the resulting path. The VFS delivers the bytes
of the `.git` directory; `git-core` provides the `[C]` navigation over them.
Neither crate learns about the other, which is the same medium-agnostic
composition ADR-0016 protects for PARSER crates.

## Consequences

- `git-core`/`git-forensic` keep a four-dependency reader and a two-dependency
  analyzer; they do not inherit the VFS contract surface, and the VFS does not
  grow a variant that fits one artifact family.
- Findings keep citing what actually identifies the evidence — the object hash
  (`Evidence { field: "commit", value: commit.to_hex(), location: None }` in
  `forensic/src/lib.rs`) — rather than a fabricated path or extent.
- **A real limitation, stated plainly:** `git-core` requires a real filesystem
  path and cannot read a `.git` from an in-memory byte source or a `DynSource`.
  Reaching one inside an image needs extraction or a FUSE mount. A byte-source
  constructor would be a `git-core` API addition, independent of this decision.
- **If `[C]` is ever wanted inside the VFS**, it is an architectural change to a
  published crate, not an adapter. It would require, at minimum:
  1. A content-addressed `Layer` variant in `forensic-vfs`
     (`crates/core/src/locator.rs`) with hash-based addressing, plus its token in
     the canonical locator URI grammar (`crates/core/src/uri.rs`, encode
     `:326-348`, decode `:361-416`, `Display` `:479-497`). That URI is a
     reproducibility record cited in reports, so extending its grammar changes a
     published format.
  2. A `[C]` navigation trait as a peer of `FileSystem`, alongside
     `ContainerOpen`/`VolumeSystemOpen`/`EncryptionOpen`/`FileSystemOpen`/`ArchiveOpen`
     (`crates/core/src/registry.rs`), with its own `Openers` slot — following the
     precedent of `forensic-vfs` ADR-0008, which gave archives their own
     first-class `ArchiveOpen` trait rather than mapping them onto an existing
     one.
  3. Widening the resolver terminal: `Resolved.fs` and `Evidence.fs`
     (`crates/resolver/src/lib.rs:53-72`) are typed `DynFs`, so a resolution that
     ends in a content store cannot be expressed without changing them.
  Scope note, stated honestly: `Layer` is `#[non_exhaustive]`
  (`crates/core/src/locator.rs:45`), so the new variant alone is additive for
  out-of-crate matches; the breaking part is the resolver terminal types and any
  new required trait surface. That work belongs in a `forensic-vfs` ADR and a
  breaking release of that crate, not here.

## Note on recovered rationale

The *state* recorded here is directly verifiable in the code: no `forensic-vfs`
dependency in either manifest or the lock, a purely hash-keyed reader API, and no
content-addressed variant in the published `Layer` enum. The fleet placement in
GRAPH NAV `[C]` is likewise pre-existing (`ronin-issen` ADR-0016, this repo's
`docs/PRD.md`). The *deliberation* — the head-to-head of adapting `FileSystem`
versus staying a direct `[C]` navigator — is written here for the first time; it
is reconstructed from the fleet primitives and the two crates' APIs, not from a
documented earlier debate in the git history.
