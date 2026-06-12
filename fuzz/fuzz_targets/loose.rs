#![no_main]
//! Loose object decompress + header parse over arbitrary bytes.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(h) = git_core::GitHash::from_bytes(&[0u8; 20]) {
        let _ = git_core::loose::decompress_and_parse(&h, data);
    }
});
