use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;
use sha1::{Digest, Sha1};

use crate::error::{GitError, Result};
use crate::hash::GitHash;
use crate::object::{ObjectKind, RawObject};

/// Maximum decompressed size accepted (deflate-bomb guard): 512 MiB.
const MAX_DECOMPRESSED_SIZE: u64 = 512 * 1024 * 1024;

/// Enumerate every loose object under `objects_dir` by scanning
/// `objects/<xx>/<38hex>`. Malformed names are skipped; never panics. A missing
/// `objects` directory simply yields an empty list.
#[must_use]
pub fn list_loose(objects_dir: &Path) -> Vec<GitHash> {
    let mut out = Vec::new();
    let Ok(shards) = std::fs::read_dir(objects_dir) else {
        return out;
    };
    for shard in shards.flatten() {
        let shard_name = shard.file_name();
        let Some(prefix) = shard_name.to_str() else {
            continue;
        };
        // Object shards are exactly two hex chars (skip `pack`, `info`, …).
        if prefix.len() != 2 || !prefix.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(shard.path()) else {
            continue;
        };
        for file in files.flatten() {
            let file_name = file.file_name();
            let Some(rest) = file_name.to_str() else {
                continue;
            };
            if rest.len() != 38 {
                continue;
            }
            let mut hex = String::with_capacity(40);
            hex.push_str(prefix);
            hex.push_str(rest);
            if let Ok(hash) = GitHash::from_hex(&hex) {
                out.push(hash);
            }
        }
    }
    out
}

/// Read and parse a loose git object file.
pub fn read_loose(objects_dir: &Path, hash: &GitHash) -> Result<RawObject> {
    let (dir, file) = hash.object_path();
    let path = objects_dir.join(&dir).join(&file);

    let compressed = std::fs::read(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            GitError::ObjectNotFound(*hash)
        } else {
            GitError::Io(e)
        }
    })?;

    decompress_and_parse(hash, &compressed)
}

pub fn decompress_and_parse(expected: &GitHash, compressed: &[u8]) -> Result<RawObject> {
    let mut decompressed = Vec::new();
    ZlibDecoder::new(compressed)
        .take(MAX_DECOMPRESSED_SIZE)
        .read_to_end(&mut decompressed)
        .map_err(|e| GitError::InvalidObject(format!("zlib decompression failed: {e}")))?;

    // SHA1 over the entire decompressed content (header + NUL + data) must equal the object hash.
    let mut hasher = Sha1::new();
    hasher.update(&decompressed);
    let digest: [u8; 20] = hasher.finalize().into();
    let got = GitHash(digest);
    let verified = got == *expected;

    // Object header: "<kind> <size>\0<data>"
    let nul = decompressed
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| GitError::InvalidObject("missing NUL in object header".into()))?;

    let header = std::str::from_utf8(&decompressed[..nul])
        .map_err(|e| GitError::InvalidObject(format!("object header not UTF-8: {e}")))?;

    let (kind_str, size_str) = header
        .split_once(' ')
        .ok_or_else(|| GitError::InvalidObject("object header missing space".into()))?;

    let kind = ObjectKind::from_bytes(kind_str.as_bytes())?;

    let declared_size: usize = size_str
        .parse()
        .map_err(|_| GitError::InvalidObject(format!("invalid size in header: {size_str:?}")))?;

    let data = decompressed[nul + 1..].to_vec();

    if data.len() != declared_size {
        return Err(GitError::InvalidObject(format!(
            "declared size {declared_size} but data is {} bytes",
            data.len()
        )));
    }

    Ok(RawObject {
        kind,
        data,
        verified,
    })
}
