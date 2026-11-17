//! Encryption at rest: XChaCha20-Poly1305 with the key from the keyring.
//!
//! Every sealed value is `nonce (24 bytes) || ciphertext || tag (16 bytes)`,
//! with a fresh random nonce each time. The associated data names what the
//! value is and which Item it belongs to, so a value moved to another row or
//! column fails to open instead of decrypting into the wrong place
//! (see `docs/adr/0005-*`).

use chacha20poly1305::aead::{Aead, Generate, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use std::fmt;
use zeroize::Zeroizing;

const NONCE_LEN: usize = 24;

/// Length of a [`Key`] in bytes.
pub const KEY_LEN: usize = 32;

/// The at-rest key that encrypts Plain Items on disk.
///
/// It lives only in the Secret Service keyring and in this process's memory
/// (ADR 0002). Like [`crate::Content`], it implements neither `Debug` nor
/// `Display`, and its bytes are zeroed when it is dropped.
pub struct Key(Zeroizing<[u8; KEY_LEN]>);

impl Key {
    /// A new random key from the operating system's generator.
    ///
    /// # Errors
    ///
    /// [`CryptoError::Random`] if the operating system can't provide randomness.
    pub fn generate() -> Result<Self, CryptoError> {
        let key = chacha20poly1305::Key::try_generate().map_err(|_| CryptoError::Random)?;
        let mut bytes = Zeroizing::new([0; KEY_LEN]);
        bytes.copy_from_slice(&key);
        Ok(Self(bytes))
    }

    /// A key read back from the keyring.
    ///
    /// # Errors
    ///
    /// [`CryptoError::KeyLength`] unless `bytes` is exactly [`KEY_LEN`] long.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let mut key = Zeroizing::new([0; KEY_LEN]);
        if bytes.len() != KEY_LEN {
            return Err(CryptoError::KeyLength);
        }
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    /// The raw key, to hand to the keyring. No other caller needs it.
    #[must_use]
    pub fn expose(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new((&*self.0).into())
    }
}

/// Why sealing or opening a value failed. Never carries the value itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// The operating system's random generator failed.
    Random,
    /// A key from the keyring has the wrong length.
    KeyLength,
    /// A sealed value is malformed, was tampered with, belongs elsewhere or
    /// was sealed with another key.
    Unreadable,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Random => "the system random number generator failed",
            Self::KeyLength => "the History key has the wrong length",
            Self::Unreadable => "a stored value could not be decrypted",
        })
    }
}

impl std::error::Error for CryptoError {}

/// Encrypts `plaintext`, binding it to `aad`.
pub(crate) fn seal(key: &Key, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let nonce = XNonce::try_generate().map_err(|_| CryptoError::Random)?;
    let ciphertext = key
        .cipher()
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CryptoError::Unreadable)?;
    let mut sealed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

/// Decrypts a value from [`seal`], checking it was sealed with the same `aad`.
pub(crate) fn open(
    key: &Key,
    aad: &[u8],
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let (nonce, ciphertext) = sealed
        .split_at_checked(NONCE_LEN)
        .ok_or(CryptoError::Unreadable)?;
    let nonce = XNonce::try_from(nonce).map_err(|_| CryptoError::Unreadable)?;
    key.cipher()
        .decrypt(
            &nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Unreadable)
}

#[cfg(test)]
mod tests {
    use super::{CryptoError, KEY_LEN, Key, NONCE_LEN, open, seal};
    use proptest::prelude::*;
    use static_assertions::assert_not_impl_any;
    use std::fmt::{Debug, Display};

    assert_not_impl_any!(Key: Debug, Display);

    fn key() -> Key {
        Key::generate().unwrap()
    }

    proptest! {
        #[test]
        fn anything_sealed_opens_to_itself(
            plaintext in proptest::collection::vec(any::<u8>(), 0..2048),
            aad in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            let key = key();
            let sealed = seal(&key, &aad, &plaintext).unwrap();
            let opened = open(&key, &aad, &sealed).unwrap();
            prop_assert_eq!(opened.as_slice(), plaintext.as_slice());
        }

        #[test]
        fn sealing_twice_differs(plaintext in proptest::collection::vec(any::<u8>(), 0..256)) {
            let key = key();
            prop_assert_ne!(seal(&key, b"a", &plaintext).unwrap(), seal(&key, b"a", &plaintext).unwrap());
        }
    }

    #[test]
    fn sealed_value_hides_the_plaintext() {
        let marker = b"COPYVEIL-MARKER-sealed";
        let sealed = seal(&key(), b"aad", marker).unwrap();
        assert_eq!(sealed.len(), NONCE_LEN + marker.len() + 16);
        assert!(!sealed.windows(marker.len()).any(|window| window == marker));
    }

    #[test]
    fn wrong_key_or_aad_is_unreadable() {
        let key = key();
        let sealed = seal(&key, b"meta:1", b"x").unwrap();
        assert_eq!(
            open(&Key::generate().unwrap(), b"meta:1", &sealed).err(),
            Some(CryptoError::Unreadable)
        );
        assert_eq!(
            open(&key, b"meta:2", &sealed).err(),
            Some(CryptoError::Unreadable)
        );
    }

    #[test]
    fn tampered_or_truncated_values_are_unreadable() {
        let key = key();
        let mut sealed = seal(&key, b"a", b"value").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert_eq!(
            open(&key, b"a", &sealed).err(),
            Some(CryptoError::Unreadable)
        );
        assert_eq!(
            open(&key, b"a", &sealed[..NONCE_LEN - 1]).err(),
            Some(CryptoError::Unreadable)
        );
        assert_eq!(
            open(&key, b"a", &sealed[..NONCE_LEN]).err(),
            Some(CryptoError::Unreadable)
        );
    }

    #[test]
    fn keys_round_trip_through_bytes() {
        let key = key();
        let copy = Key::from_bytes(key.expose()).unwrap();
        let sealed = seal(&key, b"a", b"x").unwrap();
        assert_eq!(open(&copy, b"a", &sealed).unwrap().as_slice(), b"x");
        assert_ne!(key.expose(), Key::generate().unwrap().expose());
    }

    #[test]
    fn keys_of_the_wrong_length_are_refused() {
        assert_eq!(
            Key::from_bytes(&[0; KEY_LEN - 1]).err(),
            Some(CryptoError::KeyLength)
        );
        assert_eq!(
            Key::from_bytes(&[0; KEY_LEN + 1]).err(),
            Some(CryptoError::KeyLength)
        );
    }

    #[test]
    fn errors_describe_themselves() {
        assert_eq!(
            CryptoError::Random.to_string(),
            "the system random number generator failed"
        );
        assert_eq!(
            CryptoError::KeyLength.to_string(),
            "the History key has the wrong length"
        );
        assert_eq!(
            CryptoError::Unreadable.to_string(),
            "a stored value could not be decrypted"
        );
    }
}
