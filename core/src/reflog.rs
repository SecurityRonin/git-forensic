//! Reflog reading (`.git/logs/<ref>`), git's local record of every ref movement.
//!
//! Each line has the form
//! `<oldsha40> <newsha40> <name> <email> <unix_ts> <tzoffset>\t<message>\n`.
//! The tab before the message is the reliable separator; the identity prefix is
//! parsed git-style (the email lives in `<...>`, so a name may contain spaces).
//!
//! Reference: git `Documentation/gitrevisions.txt` (reflog syntax) and
//! `Documentation/git-reflog.txt`.

use std::path::Path;

use crate::error::Result;
use crate::hash::GitHash;

/// One reflog line: a single movement of a ref from `old` to `new`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflogEntry {
    /// The ref's value before the movement (all-zero for the very first write).
    pub old: GitHash,
    /// The ref's value after the movement.
    pub new: GitHash,
    /// Name of the committer who performed the movement.
    pub name: String,
    /// Email of the committer who performed the movement.
    pub email: String,
    /// Unix timestamp (seconds since epoch) of the movement.
    pub timestamp: i64,
    /// Timezone offset as recorded (e.g. `+0800`), kept verbatim.
    pub tz_offset: String,
    /// The operation message (e.g. `commit: x`, `reset: moving to HEAD~1`).
    pub message: String,
}

/// Parse a whole reflog file's bytes into entries.
///
/// Malformed lines are skipped rather than fatal: a reflog is local, mutable,
/// and may be partially truncated, so robustness beats strictness here. This
/// never panics regardless of input.
#[must_use]
pub fn parse_reflog(bytes: &[u8]) -> Vec<ReflogEntry> {
    let text = String::from_utf8_lossy(bytes);
    text.lines().filter_map(parse_line).collect()
}

/// Parse a single reflog line, returning `None` if it is malformed.
fn parse_line(line: &str) -> Option<ReflogEntry> {
    // The message is everything after the first tab.
    let (prefix, message) = line.split_once('\t')?;

    // Prefix: "<oldsha> <newsha> <name> <email> <ts> <tz>".
    // The email is delimited by '<' .. '>', so split around it to tolerate a
    // name containing spaces.
    let email_start = prefix.find('<')?;
    let email_end = prefix[email_start..].find('>')? + email_start;

    let head = prefix.get(..email_start)?.trim_end();
    let email = prefix.get(email_start + 1..email_end)?.to_string();
    let tail = prefix.get(email_end + 1..)?.trim();

    // head = "<oldsha> <newsha> <name>"
    let mut head_parts = head.splitn(3, ' ');
    let old = GitHash::from_hex(head_parts.next()?).ok()?;
    let new = GitHash::from_hex(head_parts.next()?).ok()?;
    let name = head_parts.next()?.trim().to_string();

    // tail = "<ts> <tz>"
    let mut tail_parts = tail.split_whitespace();
    let timestamp: i64 = tail_parts.next()?.parse().ok()?;
    let tz_offset = tail_parts.next().unwrap_or("+0000").to_string();

    Some(ReflogEntry {
        old,
        new,
        name,
        email,
        timestamp,
        tz_offset,
        message: message.to_string(),
    })
}

/// Read and parse `.git/logs/<refname>` for `git_dir`.
///
/// Returns an empty vec (not an error) when the log file is absent — git only
/// creates a log once a ref has moved, so absence is normal, not a failure.
///
/// # Errors
/// Propagates a non-`NotFound` I/O error encountered reading the log file.
pub fn read_reflog(git_dir: &Path, refname: &str) -> Result<Vec<ReflogEntry>> {
    let path = git_dir.join("logs").join(refname);
    match std::fs::read(&path) {
        Ok(bytes) => Ok(parse_reflog(&bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_basic_line() {
        let e = parse_line(
            "0000000000000000000000000000000000000000 \
             3abc579ce97f2484371fbe6e52d1fa43699479b5 A <a@b.x> 100 +0000\tcommit: x",
        )
        .unwrap();
        assert_eq!(e.name, "A");
        assert_eq!(e.email, "a@b.x");
        assert_eq!(e.timestamp, 100);
        assert_eq!(e.message, "commit: x");
    }

    #[test]
    fn rejects_a_line_without_a_tab() {
        assert!(parse_line("no tab here").is_none());
    }

    #[test]
    fn rejects_a_line_without_an_email() {
        assert!(parse_line("aaa bbb name 100 +0000\tmsg").is_none());
    }

    #[test]
    fn rejects_a_short_hash() {
        assert!(parse_line("dead beef A <a@b.x> 100 +0000\tmsg").is_none());
    }

    #[test]
    fn rejects_a_non_numeric_timestamp() {
        assert!(parse_line(
            "0000000000000000000000000000000000000000 \
             3abc579ce97f2484371fbe6e52d1fa43699479b5 A <a@b.x> nope +0000\tmsg"
        )
        .is_none());
    }

    #[test]
    fn defaults_missing_tz_to_utc() {
        let e = parse_line(
            "0000000000000000000000000000000000000000 \
             3abc579ce97f2484371fbe6e52d1fa43699479b5 A <a@b.x> 100\tmsg",
        )
        .unwrap();
        assert_eq!(e.tz_offset, "+0000");
    }
}
