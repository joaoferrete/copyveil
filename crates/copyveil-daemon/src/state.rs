//! The daemon's state: the History and where its Plain Items are kept.
//!
//! Every change goes through [`State`], which applies it to the History and
//! then mirrors the History to the encrypted store. Nothing here knows about
//! D-Bus, so it is tested directly.

use copyveil_core::{Capture, History, HistoryError, ItemId, Opened, Sensitivity, Settings, Store};
use std::time::{Instant, SystemTime};

/// Where Plain Items are kept.
pub enum Storage {
    /// Encrypted on disk (ADR 0002, ADR 0005).
    Disk(Box<Store>),
    /// In memory only; the History is lost when the daemon stops.
    Memory,
}

/// What the user should be told about storage. Shown by the app; never
/// carries Item content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    /// Nothing to tell.
    None,
    /// No usable keyring, so the History is kept in memory only.
    KeyringUnavailable,
    /// The store could not be opened or read, so the History is kept in
    /// memory only.
    StoreUnavailable,
    /// The store couldn't be opened with the key and was erased.
    StoreWiped,
    /// Some stored Items couldn't be decrypted and were erased.
    ItemsUnreadable,
    /// Writing to the store failed; it is retried on the next change.
    SyncFailed,
}

impl Notice {
    /// The stable name sent over D-Bus.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "",
            Self::KeyringUnavailable => "keyring-unavailable",
            Self::StoreUnavailable => "store-unavailable",
            Self::StoreWiped => "store-wiped",
            Self::ItemsUnreadable => "items-unreadable",
            Self::SyncFailed => "sync-failed",
        }
    }

    /// The notice for how [`Store::open`] found the store.
    #[must_use]
    pub const fn for_opened(opened: Opened) -> Self {
        match opened {
            Opened::Wiped => Self::StoreWiped,
            Opened::Created | Opened::Existing => Self::None,
        }
    }
}

/// The History, its storage and the storage notice.
pub struct State {
    history: History,
    storage: Storage,
    notice: Notice,
}

impl State {
    /// Loads the History from `storage`. A store that fails to load is
    /// dropped and the daemon continues in memory only.
    pub fn load(settings: Settings, storage: Storage, notice: Notice, now: Instant) -> Self {
        match storage {
            Storage::Memory => Self {
                history: History::new(settings),
                storage: Storage::Memory,
                notice,
            },
            Storage::Disk(mut store) => {
                match store.load(settings.clone(), now, SystemTime::now()) {
                    Ok(loaded) => {
                        let notice = if loaded.unreadable > 0 && notice == Notice::None {
                            tracing::warn!(
                                count = loaded.unreadable,
                                "erased unreadable stored Items"
                            );
                            Notice::ItemsUnreadable
                        } else {
                            notice
                        };
                        let mut state = Self {
                            history: loaded.history,
                            storage: Storage::Disk(store),
                            notice,
                        };
                        // Items a newer detector now flags leave the disk now.
                        state.persist(now);
                        state
                    }
                    Err(err) => {
                        tracing::error!(%err, "could not load the History; keeping it in memory only");
                        Self {
                            history: History::new(settings),
                            storage: Storage::Memory,
                            notice: Notice::StoreUnavailable,
                        }
                    }
                }
            }
        }
    }

    /// The History.
    #[must_use]
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Whether Plain Items are kept on disk.
    #[must_use]
    pub fn on_disk(&self) -> bool {
        matches!(self.storage, Storage::Disk(_))
    }

    /// What the user should be told about storage.
    #[must_use]
    pub fn notice(&self) -> Notice {
        self.notice
    }

    /// Applies validated settings: a lower Plain limit removes Items now,
    /// and new Expiry times apply to Items captured from now on.
    pub fn set_settings(&mut self, settings: Settings, now: Instant) {
        self.history.set_settings(settings);
        self.persist(now);
    }

    /// Stops keeping the History on disk and erases the store's files. The
    /// History in memory is untouched.
    pub fn go_memory_only(&mut self) {
        if let Storage::Disk(store) = std::mem::replace(&mut self.storage, Storage::Memory)
            && let Err(err) = store.destroy()
        {
            tracing::error!(%err, "could not erase the History store");
        }
        self.notice = Notice::None;
    }

    /// Starts keeping Plain Items on disk again, writing the ones in memory
    /// now. `storage` comes from [`crate::StorageOpener::open_fresh`], so it
    /// starts empty and ends up mirroring this History.
    pub fn attach(&mut self, storage: Storage, notice: Notice, now: Instant) {
        self.storage = storage;
        self.notice = notice;
        self.persist(now);
    }

    /// Adds a Capture. See [`History::capture`].
    pub fn capture(&mut self, capture: Capture, now: Instant) -> ItemId {
        let id = self.history.capture(capture, now);
        self.persist(now);
        id
    }

    /// Removes an Item.
    ///
    /// # Errors
    ///
    /// See [`History::remove`].
    pub fn remove(&mut self, id: ItemId, now: Instant) -> Result<(), HistoryError> {
        self.history.remove(id)?;
        self.persist(now);
        Ok(())
    }

    /// Pins an Item.
    ///
    /// # Errors
    ///
    /// See [`History::pin`].
    pub fn pin(&mut self, id: ItemId, now: Instant) -> Result<(), HistoryError> {
        self.history.pin(id)?;
        self.persist(now);
        Ok(())
    }

    /// Unpins an Item.
    ///
    /// # Errors
    ///
    /// See [`History::unpin`].
    pub fn unpin(&mut self, id: ItemId, now: Instant) -> Result<(), HistoryError> {
        self.history.unpin(id)?;
        self.persist(now);
        Ok(())
    }

    /// Moves a Pin to another Pin slot.
    ///
    /// # Errors
    ///
    /// See [`History::move_pin`].
    pub fn move_pin(&mut self, id: ItemId, slot: usize, now: Instant) -> Result<(), HistoryError> {
        self.history.move_pin(id, slot)?;
        self.persist(now);
        Ok(())
    }

    /// Changes an Item's Sensitivity with an Override.
    ///
    /// # Errors
    ///
    /// See [`History::override_sensitivity`].
    pub fn override_sensitivity(
        &mut self,
        id: ItemId,
        sensitivity: Sensitivity,
        now: Instant,
    ) -> Result<(), HistoryError> {
        self.history.override_sensitivity(id, sensitivity, now)?;
        self.persist(now);
        Ok(())
    }

    /// Erases the Items whose Expiry has come. See [`History::expire`].
    pub fn expire(&mut self, now: Instant) -> Vec<ItemId> {
        let expired = self.history.expire(now);
        if !expired.is_empty() {
            self.persist(now);
        }
        expired
    }

    /// Mirrors the History to disk. A failure is logged and reported as a
    /// notice; the next change retries.
    fn persist(&mut self, now: Instant) {
        let Storage::Disk(store) = &mut self.storage else {
            return;
        };
        match store.sync(&self.history, now, SystemTime::now()) {
            Ok(()) => {
                if self.notice == Notice::SyncFailed {
                    self.notice = Notice::None;
                }
            }
            Err(err) => {
                tracing::error!(%err, "could not write the History to disk");
                self.notice = Notice::SyncFailed;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Notice, State, Storage};
    use copyveil_core::{Capture, Key, Opened, Sensitivity, Settings, Store};
    use std::time::Instant;

    #[test]
    fn notice_names_are_stable() {
        let names: Vec<_> = [
            Notice::None,
            Notice::KeyringUnavailable,
            Notice::StoreUnavailable,
            Notice::StoreWiped,
            Notice::ItemsUnreadable,
            Notice::SyncFailed,
        ]
        .iter()
        .map(|notice| notice.name())
        .collect();
        assert_eq!(
            names,
            [
                "",
                "keyring-unavailable",
                "store-unavailable",
                "store-wiped",
                "items-unreadable",
                "sync-failed"
            ]
        );
        assert_eq!(Notice::for_opened(Opened::Wiped), Notice::StoreWiped);
        assert_eq!(Notice::for_opened(Opened::Created), Notice::None);
        assert_eq!(Notice::for_opened(Opened::Existing), Notice::None);
    }

    #[test]
    fn every_change_reaches_the_store() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("history.db");
        let key = *Key::generate().unwrap().expose();
        let open = || {
            let (store, _) = Store::open(&path, Key::from_bytes(&key).unwrap()).unwrap();
            State::load(
                Settings::default(),
                Storage::Disk(Box::new(store)),
                Notice::None,
                Instant::now(),
            )
        };
        let now = Instant::now();
        let mut state = open();
        assert!(state.on_disk());
        let a = state.capture(Capture::text("a"), now);
        let b = state.capture(Capture::text("b"), now);
        let c = state.capture(Capture::text("c"), now);
        let d = state.capture(Capture::text("d"), now);
        state.pin(a, now).unwrap();
        state.pin(b, now).unwrap();
        state.move_pin(b, 1, now).unwrap();
        state.unpin(a, now).unwrap();
        state.remove(c, now).unwrap();
        state
            .override_sensitivity(d, Sensitivity::Secret, now)
            .unwrap();
        assert_eq!(state.expire(now), []);
        drop(state);

        let state = open();
        let ids: Vec<_> = state
            .history()
            .items()
            .iter()
            .map(copyveil_core::Item::id)
            .collect();
        assert_eq!(ids, [b, a]);
        assert!(state.history().is_pinned(b));
        assert!(!state.history().is_pinned(a));
        assert_eq!(state.notice(), Notice::None);
    }

    #[test]
    fn memory_state_keeps_its_notice() {
        let state = State::load(
            Settings::default(),
            Storage::Memory,
            Notice::KeyringUnavailable,
            Instant::now(),
        );
        assert!(!state.on_disk());
        assert_eq!(state.notice(), Notice::KeyringUnavailable);
    }
}
