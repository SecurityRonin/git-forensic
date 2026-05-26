use std::path::Path;
use crate::error::Result;
use crate::hash::GitHash;
use crate::object::RawObject;

pub fn read_loose(objects_dir: &Path, hash: &GitHash) -> Result<RawObject> {
    todo!()
}

pub fn decompress_and_parse(expected: &GitHash, compressed: &[u8]) -> Result<RawObject> {
    todo!()
}
