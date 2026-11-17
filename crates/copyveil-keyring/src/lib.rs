//! The History key in the Secret Service keyring (ADR 0002, ADR 0005).
//!
//! This crate is the one place Copyveil talks to the keyring. It is kept thin
//! because it can only be tested against a running Secret Service: every
//! decision about what the key protects lives in `copyveil-core`.

use copyveil_core::{CryptoError, Key};
use std::fmt;
use zeroize::Zeroizing;

/// The attributes that identify Copyveil's History key in the keyring.
pub const ATTRIBUTES: [(&str, &str); 2] = [
    ("application", "io.github.joaoferrete.Copyveil"),
    ("purpose", "history-key"),
];

/// The label the user sees in the keyring manager (Seahorse, `KWallet`).
pub const LABEL: &str = "Copyveil History key";

/// Why the History key could not be read or created. The daemon then runs
/// in memory only and tells the user why (ADR 0002).
#[derive(Debug)]
pub enum KeyringError {
    /// No Secret Service is reachable, or it refused the request.
    Unavailable(oo7::Error),
    /// The stored key is unusable (it has the wrong length).
    Malformed(CryptoError),
    /// A new key could not be generated.
    Random(CryptoError),
}

impl fmt::Display for KeyringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(err) => write!(f, "the keyring is unavailable: {err}"),
            Self::Malformed(err) => write!(f, "the History key in the keyring is unusable: {err}"),
            Self::Random(err) => write!(f, "could not create a History key: {err}"),
        }
    }
}

impl std::error::Error for KeyringError {}

impl From<oo7::Error> for KeyringError {
    fn from(err: oo7::Error) -> Self {
        Self::Unavailable(err)
    }
}

/// Reads the History key from the keyring, creating and storing a new one
/// the first time. Unlocks the keyring if needed, which may prompt the user.
///
/// # Errors
///
/// [`KeyringError`] when there is no usable key; there is never a fallback.
pub async fn history_key() -> Result<Key, KeyringError> {
    let keyring = oo7::Keyring::new().await?;
    keyring.unlock().await?;
    if let Some(item) = keyring.search_items(&ATTRIBUTES).await?.first() {
        let secret = item.secret().await?;
        return Key::from_bytes(secret.as_bytes()).map_err(KeyringError::Malformed);
    }
    let key = Key::generate().map_err(KeyringError::Random)?;
    let secret = Zeroizing::new(key.expose().to_vec());
    keyring
        .create_item(LABEL, &ATTRIBUTES, secret, false)
        .await?;
    Ok(key)
}
