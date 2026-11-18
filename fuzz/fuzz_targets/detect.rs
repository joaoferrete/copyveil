//! Every detector, on any text: must never panic.
#![no_main]

use copyveil_detect::{DetectorSet, detect};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = detect(&text, DetectorSet::all());
});
