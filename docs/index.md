# git-forensic

A from-scratch Git object-store reader and a graded anomaly auditor — read loose
and packfile objects from a `.git` produced on **any** OS, then surface the
backdated commits, rewritten history, unsigned commits in a signed history, and
unreachable objects that a normal `git log` is built to hide.

Two crates, one workspace:

- **[`git-core`](https://crates.io/crates/git-core)** — the reader: loose +
  packfile (v2, OFS/REF delta) objects, refs, `packed-refs`, reflog, commits, and
  trees over the content-addressed Merkle DAG, on any `Read + Seek`-style path. No
  `unsafe`, no `libgit2`, no C bindings. Reads a Windows-, macOS-, or Linux-authored
  `.git` identically (object format is OS-agnostic; ref names stay slash-canonical;
  CRLF control files parse).
- **[`git-forensic`](https://crates.io/crates/git-forensic)** — the auditor: turns
  parsed objects into severity-graded
  [`forensicnomicon::report::Finding`](https://crates.io/crates/forensicnomicon)s
  so a repository's anomalies aggregate uniformly with the rest of the fleet.

## Audit a repository

```rust
use git_forensic::{audit_repo, source};
use git_core::GitRepo;
use std::path::Path;

let repo = GitRepo::open(Path::new("/path/to/.git"))?;
let head = repo.head()?;

for anomaly in audit_repo(&repo, head)? {
    let finding = anomaly.to_finding(source("repo"));
    println!("[{:?}] {} — {}", finding.severity, finding.code, finding.note);
    // e.g. [Some(Medium)] GIT-COMMIT-TIME-INVERSION — committer timestamp … precedes author …
}
# Ok::<(), git_core::GitError>(())
```

## The anomaly codes

Each anomaly is an **observation** ("consistent with …"); the examiner draws the
conclusions. Codes are a stable, published contract.

| Code | Severity | What it observes |
|---|---|---|
| `GIT-COMMIT-TIME-INVERSION` | Medium | A commit's author/committer time precedes its parent's — backdating, consistent with a forged commit date |
| `GIT-HISTORY-REWRITE` | Medium | Reflog entries whose old→new transition is non-fast-forward — a force-push / rebase that rewrote published history |
| `GIT-UNSIGNED-IN-SIGNED-HISTORY` | Medium | An unsigned commit interleaved into an otherwise GPG/gitsign-signed history |
| `GIT-UNREACHABLE-OBJECT` | Medium (commit) / Low (blob, tree) | An object present in the store but reachable from no ref — a dangling/recoverable artifact |

## Trust but verify

`git-core` is panic-free on untrusted input (bounds-checked reads, no `unwrap`
in production), fuzzed per parsed structure, and validated against repositories
produced by the real `git` binary (Doer-Checker). A `.git` is attacker-controllable
evidence; the reader treats it as such.

---

[Privacy Policy](https://securityronin.github.io/git-forensic/privacy/) · [Terms of Service](https://securityronin.github.io/git-forensic/terms/) · © 2026 Security Ronin Ltd
