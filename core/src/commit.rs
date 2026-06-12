use crate::error::{GitError, Result};
use crate::hash::GitHash;

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub email: String,
    /// Unix timestamp (seconds since epoch).
    pub timestamp: i64,
    /// UTC offset in seconds (e.g. +0800 → 28800).
    pub tz_offset_secs: i32,
}

#[derive(Debug, Clone)]
pub struct CommitObject {
    pub hash: GitHash,
    pub tree: GitHash,
    pub parents: Vec<GitHash>,
    pub author: Signature,
    pub committer: Signature,
    pub message: String,
    /// True iff a `gpgsig` header was present (the commit is cryptographically
    /// signed). The signature's validity is not checked here — only its
    /// presence, which is what a signing-policy audit reasons about.
    pub is_signed: bool,
}

impl CommitObject {
    /// Parse the raw commit object bytes (after the object header is stripped).
    pub fn parse(hash: GitHash, data: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(data)
            .map_err(|e| GitError::InvalidObject(format!("commit not valid UTF-8: {e}")))?;

        let mut tree = None;
        let mut parents = Vec::new();
        let mut author = None;
        let mut committer = None;
        let mut is_signed = false;
        let mut message_start = text.len();

        for (i, line) in text.lines().enumerate() {
            if line.is_empty() {
                // Blank line separates header from message body.
                let byte_pos = text
                    .char_indices()
                    .filter(|(_, c)| *c == '\n')
                    .nth(i)
                    .map(|(pos, _)| pos + 1)
                    .unwrap_or(text.len());
                message_start = byte_pos;
                break;
            }
            if let Some(rest) = line.strip_prefix("tree ") {
                tree = Some(GitHash::from_hex(rest.trim())?);
            } else if let Some(rest) = line.strip_prefix("parent ") {
                parents.push(GitHash::from_hex(rest.trim())?);
            } else if let Some(rest) = line.strip_prefix("author ") {
                author = Some(parse_signature(rest)?);
            } else if let Some(rest) = line.strip_prefix("committer ") {
                committer = Some(parse_signature(rest)?);
            } else if line.strip_prefix("gpgsig ").is_some() {
                // A signed commit carries `gpgsig <signature>`; its continuation
                // lines start with a single space and match no header prefix, so
                // they fall through harmlessly. We record only the presence —
                // signature validity is out of scope for the reader.
                is_signed = true;
            }
        }

        Ok(Self {
            hash,
            tree: tree.ok_or_else(|| GitError::InvalidObject("commit missing tree".into()))?,
            parents,
            author: author
                .ok_or_else(|| GitError::InvalidObject("commit missing author".into()))?,
            committer: committer
                .ok_or_else(|| GitError::InvalidObject("commit missing committer".into()))?,
            message: text[message_start..].to_string(),
            is_signed,
        })
    }
}

fn parse_signature(s: &str) -> Result<Signature> {
    // Format: "Name <email> timestamp tz_offset"
    let err = || GitError::InvalidObject(format!("invalid signature: {s:?}"));

    let email_end = s.rfind('>').ok_or_else(err)?;
    let email_start = s.rfind('<').ok_or_else(err)?;
    if email_start >= email_end {
        return Err(err());
    }
    let name = s[..email_start].trim().to_string();
    let email = s[email_start + 1..email_end].to_string();

    let rest = s[email_end + 1..].trim();
    let mut parts = rest.split_whitespace();
    let ts: i64 = parts.next().and_then(|t| t.parse().ok()).ok_or_else(err)?;
    let tz = parts.next().unwrap_or("+0000");

    let sign = if tz.starts_with('-') { -1i32 } else { 1 };
    let tz_digits = tz.trim_start_matches(['+', '-']);
    let tz_offset_secs = if tz_digits.len() == 4 {
        let hh: i32 = tz_digits[..2].parse().unwrap_or(0);
        let mm: i32 = tz_digits[2..].parse().unwrap_or(0);
        sign * (hh * 3600 + mm * 60)
    } else {
        0
    };

    Ok(Signature {
        name,
        email,
        timestamp: ts,
        tz_offset_secs,
    })
}
