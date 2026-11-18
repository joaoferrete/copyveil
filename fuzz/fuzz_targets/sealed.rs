//! Opening sealed values: must never panic, and no forged value may open.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| copyveil_core::fuzzing::open_sealed(data));
