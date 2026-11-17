//! Copyveil core: the History, encryption at rest and Sensitivity rules.
//!
//! This crate is desktop-agnostic. Capture sources hand Items to it; it never
//! talks to a compositor directly (see `docs/adr/0001-*`).
//!
//! ```
//! use copyveil_core::{Capture, History, Sensitivity, Settings};
//! use std::time::{Duration, Instant};
//!
//! let mut history = History::new(Settings::default());
//! let now = Instant::now();
//! let secret = history.capture(Capture::secret_text("correct-horse"), now);
//! history.capture(Capture::text("git status"), now);
//!
//! assert_eq!(history.get(secret).unwrap().sensitivity(), Sensitivity::Secret);
//! history.expire(now + Duration::from_secs(30));
//! assert!(history.get(secret).is_none());
//! ```

mod classify;
mod content;
mod crypto;
mod history;
mod item;
mod record;
mod sensitivity;
mod store;

pub use content::Content;
pub use crypto::{CryptoError, KEY_LEN, Key};
pub use history::{History, HistoryError, Settings};
pub use item::{Capture, Item, ItemId, Kind, Reason};
pub use sensitivity::Sensitivity;
pub use store::{Loaded, MAX_PERSISTED_BYTES, Opened, Store, StoreError};
