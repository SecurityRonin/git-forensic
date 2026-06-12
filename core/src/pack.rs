//! Packfile reading (pack v2 + index v2), the normal post-`gc`/clone storage.
//!
//! A `.idx` (version 2) maps object hashes to byte offsets in the sibling
//! `.pack`; each pack object is a varint-headed, zlib-compressed blob that may be
//! whole (commit/tree/blob/tag) or a delta (`OFS_DELTA`/`REF_DELTA`) against
//! another object. This module resolves deltas recursively and verifies the
//! SHA-1 of every reconstructed object.
//!
//! Reference: git `Documentation/gitformat-pack.txt` (pack + index v2 layouts,
//! delta instruction encoding).

use std::fs;
use std::io::Read;
use std::path::Path;

use flate2::read::ZlibDecoder;
use sha1::{Digest, Sha1};

use crate::error::{GitError, Result};
use crate::hash::GitHash;
use crate::object::{ObjectKind, RawObject};

/// `.idx` version-2 magic: `\377tOc`.
const IDX_V2_MAGIC: [u8; 4] = [0xff, 0x74, 0x4f, 0x63];
/// Deflate-bomb guard: reject any object decompressing past 512 MiB.
const MAX_OBJECT_SIZE: u64 = 512 * 1024 * 1024;
/// Guard against a pathological delta-base chain (cycles / fuzzed packs).
const MAX_DELTA_DEPTH: u32 = 64;

/// Read `hash` from any packfile under `objects_dir`. Returns `Ok(None)` when no
/// pack contains it (so the caller can report a genuine not-found).
pub fn read_packed(objects_dir: &Path, hash: &GitHash) -> Result<Option<RawObject>> {
    let pack_dir = objects_dir.join("pack");
    let Ok(entries) = fs::read_dir(&pack_dir) else {
        return Ok(None);
    };
    let mut idx_paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "idx"))
        .collect();
    idx_paths.sort(); // deterministic order across packs
    for idx_path in idx_paths {
        let idx = fs::read(&idx_path)?;
        if let Some(offset) = idx_lookup(&idx, hash)? {
            let pack = fs::read(idx_path.with_extension("pack"))?;
            let (kind, data) = read_object_at(&pack, offset, objects_dir, 0)?;
            let verified = verify(hash, kind, &data);
            return Ok(Some(RawObject { kind, data, verified }));
        }
    }
    Ok(None)
}

/// Binary-search an index-v2 fanout/name table for `hash`, returning its pack
/// byte offset. Non-v2 indexes fail loud rather than silently miss.
fn idx_lookup(idx: &[u8], hash: &GitHash) -> Result<Option<u64>> {
    if idx.len() < 8 || idx[0..4] != IDX_V2_MAGIC {
        // A v1 index (no magic) or anything else: don't guess.
        return Err(GitError::PackfileUnsupported(*hash));
    }
    if be_u32(idx, 4)? != 2 {
        return Err(GitError::PackfileUnsupported(*hash));
    }
    const FANOUT: usize = 8;
    let want = hash.as_bytes();
    let first = want[0] as usize;
    let mut lo = if first == 0 { 0 } else { be_u32(idx, FANOUT + (first - 1) * 4)? } as usize;
    let mut hi = be_u32(idx, FANOUT + first * 4)? as usize;
    let count = be_u32(idx, FANOUT + 255 * 4)? as usize;
    let names = FANOUT + 256 * 4;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let name = idx.get(names + mid * 20..names + mid * 20 + 20).ok_or(GitError::OutOfBounds)?;
        match name.cmp(want.as_slice()) {
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => {
                // names(count*20) then CRCs(count*4) then small offsets(count*4).
                let offsets = names + count * 20 + count * 4;
                let raw = be_u32(idx, offsets + mid * 4)?;
                if raw & 0x8000_0000 == 0 {
                    return Ok(Some(u64::from(raw)));
                }
                // MSB set: index into the 8-byte large-offset table.
                let large = offsets + count * 4;
                let slot = (raw & 0x7fff_ffff) as usize;
                return Ok(Some(be_u64(idx, large + slot * 8)?));
            }
        }
    }
    Ok(None)
}

/// Read and fully resolve the object at `offset` in `pack`.
fn read_object_at(
    pack: &[u8],
    offset: u64,
    objects_dir: &Path,
    depth: u32,
) -> Result<(ObjectKind, Vec<u8>)> {
    if depth > MAX_DELTA_DEPTH {
        return Err(GitError::InvalidObject("delta chain too deep".into()));
    }
    let off = usize::try_from(offset).map_err(|_| GitError::OutOfBounds)?;
    let (type_id, _size, body) = parse_object_header(pack, off)?;
    match type_id {
        1..=4 => Ok((kind_from_type(type_id)?, inflate(pack, body)?)),
        6 => {
            // OFS_DELTA: a negative offset (varint) to the base, then the delta.
            let (back, delta_start) = parse_ofs_delta(pack, body)?;
            let base_off = off.checked_sub(back).ok_or(GitError::OutOfBounds)?;
            let (kind, base) = read_object_at(pack, base_off as u64, objects_dir, depth + 1)?;
            Ok((kind, apply_delta(&base, &inflate(pack, delta_start)?)?))
        }
        7 => {
            // REF_DELTA: a 20-byte base hash, then the delta.
            let base_hash = GitHash::from_bytes(
                pack.get(body..body + 20).ok_or(GitError::OutOfBounds)?,
            )?;
            let delta = inflate(pack, body + 20)?;
            let (kind, base) = resolve_base(objects_dir, &base_hash, depth + 1)?;
            Ok((kind, apply_delta(&base, &delta)?))
        }
        other => Err(GitError::InvalidObject(format!("unknown pack object type {other}"))),
    }
}

/// Resolve a REF_DELTA base by hash — it may live in a pack or as a loose object.
fn resolve_base(objects_dir: &Path, hash: &GitHash, depth: u32) -> Result<(ObjectKind, Vec<u8>)> {
    if depth > MAX_DELTA_DEPTH {
        return Err(GitError::InvalidObject("delta chain too deep".into()));
    }
    if let Some(obj) = read_packed(objects_dir, hash)? {
        return Ok((obj.kind, obj.data));
    }
    let obj = crate::loose::read_loose(objects_dir, hash)?;
    Ok((obj.kind, obj.data))
}

/// Parse a pack object header: 3-bit type + variable-length size. Returns
/// `(type_id, size, body_offset)`.
fn parse_object_header(pack: &[u8], off: usize) -> Result<(u8, usize, usize)> {
    let mut pos = off;
    let b = *pack.get(pos).ok_or(GitError::OutOfBounds)?;
    pos += 1;
    let type_id = (b >> 4) & 0x7;
    let mut size = (b & 0x0f) as usize;
    let mut shift = 4;
    let mut more = b & 0x80 != 0;
    while more {
        let b = *pack.get(pos).ok_or(GitError::OutOfBounds)?;
        pos += 1;
        size |= ((b & 0x7f) as usize).checked_shl(shift).ok_or(GitError::OutOfBounds)?;
        shift += 7;
        more = b & 0x80 != 0;
    }
    Ok((type_id, size, pos))
}

/// Parse the OFS_DELTA negative base offset. Returns `(distance, body_offset)`.
fn parse_ofs_delta(pack: &[u8], off: usize) -> Result<(usize, usize)> {
    let mut pos = off;
    let mut b = *pack.get(pos).ok_or(GitError::OutOfBounds)?;
    pos += 1;
    let mut value = (b & 0x7f) as usize;
    while b & 0x80 != 0 {
        b = *pack.get(pos).ok_or(GitError::OutOfBounds)?;
        pos += 1;
        value = value
            .checked_add(1)
            .and_then(|v| v.checked_shl(7))
            .map(|v| v | (b & 0x7f) as usize)
            .ok_or(GitError::OutOfBounds)?;
    }
    Ok((value, pos))
}

/// Apply a git delta (`base` + delta instructions → reconstructed object).
fn apply_delta(base: &[u8], delta: &[u8]) -> Result<Vec<u8>> {
    let (_src, mut p) = read_delta_size(delta, 0)?;
    let (target_size, q) = read_delta_size(delta, p)?;
    p = q;
    let mut out = Vec::with_capacity(target_size.min(MAX_OBJECT_SIZE as usize));
    while p < delta.len() {
        let cmd = delta[p];
        p += 1;
        if cmd & 0x80 != 0 {
            // COPY <offset,size> from base; bytes present per the cmd bitmask.
            let mut copy_off = 0usize;
            for i in 0..4 {
                if cmd & (1 << i) != 0 {
                    copy_off |= (*delta.get(p).ok_or(GitError::OutOfBounds)? as usize) << (8 * i);
                    p += 1;
                }
            }
            let mut copy_size = 0usize;
            for i in 0..3 {
                if cmd & (1 << (4 + i)) != 0 {
                    copy_size |= (*delta.get(p).ok_or(GitError::OutOfBounds)? as usize) << (8 * i);
                    p += 1;
                }
            }
            if copy_size == 0 {
                copy_size = 0x1_0000;
            }
            let end = copy_off.checked_add(copy_size).ok_or(GitError::OutOfBounds)?;
            out.extend_from_slice(base.get(copy_off..end).ok_or(GitError::OutOfBounds)?);
        } else if cmd != 0 {
            // INSERT: `cmd` literal bytes follow in the delta stream.
            let n = cmd as usize;
            out.extend_from_slice(delta.get(p..p + n).ok_or(GitError::OutOfBounds)?);
            p += n;
        } else {
            return Err(GitError::InvalidObject("delta opcode 0 is reserved".into()));
        }
    }
    if out.len() != target_size {
        return Err(GitError::InvalidObject(format!(
            "delta produced {} bytes, header said {target_size}",
            out.len()
        )));
    }
    Ok(out)
}

/// Read a little-endian 7-bit varint (delta source/target sizes).
fn read_delta_size(delta: &[u8], mut p: usize) -> Result<(usize, usize)> {
    let mut value = 0usize;
    let mut shift = 0u32;
    loop {
        let b = *delta.get(p).ok_or(GitError::OutOfBounds)?;
        p += 1;
        value |= ((b & 0x7f) as usize).checked_shl(shift).ok_or(GitError::OutOfBounds)?;
        shift += 7;
        if b & 0x80 == 0 {
            break;
        }
    }
    Ok((value, p))
}

/// Inflate the zlib stream beginning at `start`, with a bomb guard.
fn inflate(pack: &[u8], start: usize) -> Result<Vec<u8>> {
    let compressed = pack.get(start..).ok_or(GitError::OutOfBounds)?;
    let mut out = Vec::new();
    ZlibDecoder::new(compressed)
        .take(MAX_OBJECT_SIZE + 1)
        .read_to_end(&mut out)
        .map_err(|e| GitError::InvalidObject(format!("pack zlib failed: {e}")))?;
    if out.len() as u64 > MAX_OBJECT_SIZE {
        return Err(GitError::DeflateBomb);
    }
    Ok(out)
}

fn kind_from_type(type_id: u8) -> Result<ObjectKind> {
    match type_id {
        1 => Ok(ObjectKind::Commit),
        2 => Ok(ObjectKind::Tree),
        3 => Ok(ObjectKind::Blob),
        4 => Ok(ObjectKind::Tag),
        other => Err(GitError::InvalidObject(format!("pack object type {other} is not a base type"))),
    }
}

fn kind_str(kind: ObjectKind) -> &'static [u8] {
    match kind {
        ObjectKind::Commit => b"commit",
        ObjectKind::Tree => b"tree",
        ObjectKind::Blob => b"blob",
        ObjectKind::Tag => b"tag",
    }
}

/// SHA-1 over `"<kind> <len>\0<data>"` must equal the object's name.
fn verify(hash: &GitHash, kind: ObjectKind, data: &[u8]) -> bool {
    let mut h = Sha1::new();
    h.update(kind_str(kind));
    h.update(b" ");
    h.update(data.len().to_string().as_bytes());
    h.update(b"\0");
    h.update(data);
    let digest: [u8; 20] = h.finalize().into();
    &digest == hash.as_bytes()
}

fn be_u32(b: &[u8], off: usize) -> Result<u32> {
    let s = b.get(off..off + 4).ok_or(GitError::OutOfBounds)?;
    Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn be_u64(b: &[u8], off: usize) -> Result<u64> {
    let s = b.get(off..off + 8).ok_or(GitError::OutOfBounds)?;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Ok(u64::from_be_bytes(a))
}
