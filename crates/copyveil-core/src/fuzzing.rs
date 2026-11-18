//! Entry points for the fuzz targets in `fuzz/`. Built only with the
//! `fuzzing` feature, which no shipped binary enables.

use crate::crypto::{self, Key};
use crate::record::{Body, Meta};

/// Decodes `bytes` as both stored records. Must never panic.
pub fn decode_records(bytes: &[u8]) {
    if let Ok(meta) = Meta::decode(bytes) {
        // Whatever decodes must encode back to the same bytes.
        assert_eq!(meta.encode(), bytes, "meta re-encodes differently");
    }
    if let Ok(body) = Body::decode(bytes) {
        let encoded = Body::encode(&body.content, body.plain_fallback.as_ref());
        assert_eq!(encoded.as_slice(), bytes, "body re-encodes differently");
    }
}

/// Opens `bytes` as a sealed value under a fixed key. Must never panic, and
/// must never succeed: forging a value without the key is the attack.
pub fn open_sealed(bytes: &[u8]) {
    let Ok(key) = Key::from_bytes(&[7; crypto::KEY_LEN]) else {
        return;
    };
    assert!(
        crypto::open(&key, b"copyveil/v1/meta/fuzz", bytes).is_err(),
        "a forged value opened"
    );
}
