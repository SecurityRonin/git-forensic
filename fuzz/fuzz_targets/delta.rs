#![no_main]
//! Git delta application — fully attacker-controlled delta against a base.
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Split arbitrary input into (base, delta) and apply; must never panic.
    let split = data.len() / 2;
    let (base, delta) = data.split_at(split);
    let _ = git_core::pack::apply_delta(base, delta);
});
