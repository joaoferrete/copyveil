//! The store's record decoding: must never panic, and whatever decodes must
//! encode back to the same bytes.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| copyveil_core::fuzzing::decode_records(data));
