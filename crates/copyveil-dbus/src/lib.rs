//! Copyveil's D-Bus contract, shared by the daemon and its clients.
//!
//! This crate is the API in [`docs/dbus.md`] as types: the names on the bus,
//! the Item summary `List` returns, the strings that name kinds,
//! Sensitivities and storage notices, and proxies for clients. It holds no
//! logic and no Item content, so the app can depend on it without pulling in
//! the core.
//!
//! [`docs/dbus.md`]: https://github.com/joaoferrete/copyveil/blob/main/docs/dbus.md

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// The daemon's well-known name on the session bus.
pub const BUS_NAME: &str = "io.github.joaoferrete.Copyveil.Daemon";
/// The object that implements both interfaces.
pub const OBJECT_PATH: &str = "/io/github/joaoferrete/Copyveil/Daemon";
/// The interface the app uses.
pub const HISTORY_INTERFACE: &str = "io.github.joaoferrete.Copyveil.History1";
/// The interface capture sources use.
pub const CAPTURE_INTERFACE: &str = "io.github.joaoferrete.Copyveil.Capture1";
/// The interface the settings window uses.
pub const SETTINGS_INTERFACE: &str = "io.github.joaoferrete.Copyveil.Settings1";
/// Prefix of every error name the daemon returns.
pub const ERROR_PREFIX: &str = "io.github.joaoferrete.Copyveil.Error";

/// Names of Item kinds in [`ItemSummary::kind`].
pub mod kind {
    /// Plain text.
    pub const TEXT: &str = "text";
    /// Rich text (HTML), with a plain-text fallback.
    pub const RICH_TEXT: &str = "rich-text";
    /// An image; [`super::ItemSummary::mime`] says which type.
    pub const IMAGE: &str = "image";
}

/// Names of Sensitivities in [`ItemSummary::sensitivity`] and `Override`.
pub mod sensitivity {
    /// No indication of being a secret.
    pub const PLAIN: &str = "plain";
    /// Looks like a secret.
    pub const SUSPECTED: &str = "suspected";
    /// Declared secret by its source.
    pub const SECRET: &str = "secret";
}

/// Names of the `StorageNotice` property's values. Empty means nothing to tell.
pub mod notice {
    /// No usable keyring; the History is kept in memory only.
    pub const KEYRING_UNAVAILABLE: &str = "keyring-unavailable";
    /// The store couldn't be opened; the History is kept in memory only.
    pub const STORE_UNAVAILABLE: &str = "store-unavailable";
    /// The store couldn't be opened with the key and was erased.
    pub const STORE_WIPED: &str = "store-wiped";
    /// Some stored Items couldn't be decrypted and were erased.
    pub const ITEMS_UNREADABLE: &str = "items-unreadable";
    /// Writing to disk failed; the next change retries.
    pub const SYNC_FAILED: &str = "sync-failed";
}

/// One Item as `History1.List` returns it. D-Bus signature `(tssssuttts)`.
///
/// It never holds the content of a Secret or Suspected Item: `preview` is
/// empty for them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ItemSummary {
    /// The Item id.
    pub id: u64,
    /// One of the names in [`kind`].
    pub kind: String,
    /// The MIME type of an image; empty otherwise.
    pub mime: String,
    /// One of the names in [`sensitivity`].
    pub sensitivity: String,
    /// Why it has its Sensitivity ("Looks like a GitHub token"), or empty.
    pub reason: String,
    /// Pin slot counting from 1, or 0 when not pinned.
    pub pin_slot: u32,
    /// Milliseconds since the Item was last copied.
    pub age_ms: u64,
    /// Milliseconds until its Expiry, or 0 for Plain Items.
    pub expires_in_ms: u64,
    /// Size of the content in bytes.
    pub size: u64,
    /// The start of a Plain Item's text, for display. Always empty for
    /// Secret and Suspected Items and for images.
    pub preview: String,
}

/// Client side of `io.github.joaoferrete.Copyveil.History1`.
#[zbus::proxy(
    interface = "io.github.joaoferrete.Copyveil.History1",
    default_service = "io.github.joaoferrete.Copyveil.Daemon",
    default_path = "/io/github/joaoferrete/Copyveil/Daemon"
)]
pub trait History {
    /// Every Item, newest first.
    fn list(&self) -> zbus::Result<Vec<ItemSummary>>;
    /// The first and last four characters of each Suspected text Item of at
    /// least 20 characters, by id. Never for Secret Items.
    fn suspected_ends(&self) -> zbus::Result<std::collections::HashMap<u64, (String, String)>>;
    /// Ids of the Items matching `query`, newest first.
    fn search(&self, query: &str) -> zbus::Result<Vec<u64>>;
    /// One Item's content and plain-text fallback, for a Reveal or a paste.
    fn content(&self, id: u64) -> zbus::Result<(Vec<u8>, Vec<u8>)>;
    /// The id of the Item in Pin slot `slot`, counting from 1.
    fn pin_slot(&self, slot: u32) -> zbus::Result<u64>;
    /// Asks the capture source to paste the Item.
    fn paste(&self, id: u64) -> zbus::Result<()>;
    /// Removes the Item.
    fn remove(&self, id: u64) -> zbus::Result<()>;
    /// Pins a Plain Item.
    fn pin(&self, id: u64) -> zbus::Result<()>;
    /// Unpins the Item.
    fn unpin(&self, id: u64) -> zbus::Result<()>;
    /// Moves a pinned Item to Pin slot `slot`.
    fn move_pin(&self, id: u64, slot: u32) -> zbus::Result<()>;
    /// Changes the Item's Sensitivity (a name from [`sensitivity`]).
    fn r#override(&self, id: u64, sensitivity: &str) -> zbus::Result<()>;
    /// The History changed.
    #[zbus(signal)]
    fn changed(&self) -> zbus::Result<()>;
    /// Whether Plain Items are kept on disk.
    #[zbus(property)]
    fn on_disk(&self) -> zbus::Result<bool>;
    /// A name from [`notice`], or empty.
    #[zbus(property)]
    fn storage_notice(&self) -> zbus::Result<String>;
}

/// Client side of `io.github.joaoferrete.Copyveil.Capture1`.
#[zbus::proxy(
    interface = "io.github.joaoferrete.Copyveil.Capture1",
    default_service = "io.github.joaoferrete.Copyveil.Daemon",
    default_path = "/io/github/joaoferrete/Copyveil/Daemon"
)]
pub trait Capture {
    /// Reports a clipboard change and returns the Item's id.
    fn capture(
        &self,
        mime: &str,
        content: Vec<u8>,
        plain_fallback: Vec<u8>,
        password_manager_hint: bool,
    ) -> zbus::Result<u64>;
    /// These Items reached their Expiry.
    #[zbus(signal)]
    fn expired(&self, ids: Vec<u64>) -> zbus::Result<()>;
    /// The user asked to paste this Item.
    #[zbus(signal)]
    fn paste_requested(&self, id: u64) -> zbus::Result<()>;
}

/// Client side of `io.github.joaoferrete.Copyveil.Settings1`. The daemon
/// validates every value against its limits and saves it.
#[zbus::proxy(
    interface = "io.github.joaoferrete.Copyveil.Settings1",
    default_service = "io.github.joaoferrete.Copyveil.Daemon",
    default_path = "/io/github/joaoferrete/Copyveil/Daemon"
)]
pub trait Settings {
    /// `(secret_min_s, secret_max_s, suspected_min_s, suspected_max_s, items_min, items_max)`.
    fn limits(&self) -> zbus::Result<(u32, u32, u32, u32, u32, u32)>;
    /// Every detector: `(name, title, description)`.
    fn detectors(&self) -> zbus::Result<Vec<(String, String, String)>>;
    /// Seconds a Secret lives before its Expiry.
    #[zbus(property)]
    fn secret_expiry(&self) -> zbus::Result<u32>;
    /// Sets it; refused outside the limits.
    #[zbus(property)]
    fn set_secret_expiry(&self, value: u32) -> zbus::Result<()>;
    /// Seconds a Suspected Item lives before its Expiry.
    #[zbus(property)]
    fn suspected_expiry(&self) -> zbus::Result<u32>;
    /// Sets it; refused outside the limits.
    #[zbus(property)]
    fn set_suspected_expiry(&self, value: u32) -> zbus::Result<()>;
    /// How many unpinned Plain Items the History keeps.
    #[zbus(property)]
    fn max_plain_items(&self) -> zbus::Result<u32>;
    /// Sets it; refused outside the limits.
    #[zbus(property)]
    fn set_max_plain_items(&self, value: u32) -> zbus::Result<()>;
    /// Names of the detectors turned off.
    #[zbus(property)]
    fn disabled_detectors(&self) -> zbus::Result<Vec<String>>;
    /// Sets them; unknown names are refused.
    #[zbus(property)]
    fn set_disabled_detectors(&self, value: Vec<String>) -> zbus::Result<()>;
    /// Whether the History is kept in memory only.
    #[zbus(property)]
    fn memory_only(&self) -> zbus::Result<bool>;
    /// Turning it on erases the saved History and its key.
    #[zbus(property)]
    fn set_memory_only(&self, value: bool) -> zbus::Result<()>;
}
