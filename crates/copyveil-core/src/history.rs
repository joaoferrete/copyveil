//! The History: every Item the user can browse and paste again.
//!
//! This is the in-memory model and its rules; nothing here touches the disk.
//! [`crate::Store`] mirrors its Plain Items to disk.

use crate::Sensitivity;
use crate::classify::classify;
use crate::item::{Capture, Item, ItemId, Kind, Reason};
use crate::settings::Settings;
use std::fmt;
use std::time::Instant;

/// Why a History operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryError {
    /// No Item has this id (it may have expired or been removed).
    UnknownItem(ItemId),
    /// Only Plain Items can be pinned.
    NotPlain(ItemId),
    /// The Item is not pinned.
    NotPinned(ItemId),
    /// There is no Pin slot with this number.
    NoSuchPinSlot(usize),
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownItem(id) => write!(f, "no Item with id {}", id.0),
            Self::NotPlain(id) => write!(f, "Item {} is not Plain, so it can't be pinned", id.0),
            Self::NotPinned(id) => write!(f, "Item {} is not pinned", id.0),
            Self::NoSuchPinSlot(slot) => write!(f, "there is no Pin slot {slot}"),
        }
    }
}

impl std::error::Error for HistoryError {}

/// The ordered collection of Items, newest first.
#[derive(Debug, Default)]
pub struct History {
    items: Vec<Item>,
    /// Pinned Item ids in Pin slot order: `pins[0]` is Pin slot 1.
    pins: Vec<ItemId>,
    next_id: u64,
    settings: Settings,
}

impl History {
    /// An empty History.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    /// A History rebuilt by the store: Items newest first, Pins in Pin slot
    /// order. Expiry starts from `now` for any Item no longer Plain, which
    /// also loses its Pin. New Items get ids after the largest restored one.
    pub(crate) fn restore(
        settings: Settings,
        mut items: Vec<Item>,
        mut pins: Vec<ItemId>,
        now: Instant,
    ) -> Self {
        let next_id = items.iter().map(|item| item.id.0).max().unwrap_or(0);
        let mut history = Self {
            next_id,
            settings,
            ..Self::default()
        };
        for item in &mut items {
            item.expires_at = history.expiry_for(item.sensitivity, now);
        }
        pins.retain(|id| {
            items
                .iter()
                .any(|item| item.id == *id && item.sensitivity.may_pin())
        });
        history.items = items;
        history.pins = pins;
        history.evict();
        history
    }

    /// The current settings.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Replaces the settings. Expiry times already set are kept; the Plain
    /// limit applies immediately.
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
        self.evict();
    }

    /// Adds a Capture to the top of the History and returns its Item's id.
    ///
    /// Copying content that is already in the History moves that Item to the
    /// top and restarts its Expiry instead of adding a duplicate. It keeps its
    /// id, Pin and Sensitivity (including an Override), unless a password
    /// manager now flags it as a Secret.
    pub fn capture(&mut self, capture: Capture, now: Instant) -> ItemId {
        let (mut sensitivity, mut reason) = classify(&capture, self.settings.detectors);
        let existing = self
            .items
            .iter()
            .position(|item| item.kind == capture.kind && item.content == capture.content)
            .map(|index| self.items.remove(index));
        let id = match existing {
            Some(existing) => {
                if !capture.password_manager_hint {
                    sensitivity = existing.sensitivity;
                    reason = existing.reason;
                }
                existing.id
            }
            None => self.new_id(),
        };
        if !sensitivity.may_pin() {
            self.pins.retain(|pinned| *pinned != id);
        }
        self.items.insert(
            0,
            Item {
                id,
                content: capture.content,
                kind: capture.kind,
                plain_fallback: capture.plain_fallback,
                sensitivity,
                reason,
                captured_at: now,
                expires_at: self.expiry_for(sensitivity, now),
            },
        );
        self.evict();
        id
    }

    /// Every Item, newest first.
    #[must_use]
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The Item with this id.
    #[must_use]
    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|item| item.id == id)
    }

    /// Removes an Item. Its content is zeroed as it is dropped.
    ///
    /// # Errors
    ///
    /// [`HistoryError::UnknownItem`] if there is no such Item.
    pub fn remove(&mut self, id: ItemId) -> Result<(), HistoryError> {
        let index = self.index_of(id)?;
        self.items.remove(index);
        self.pins.retain(|pinned| *pinned != id);
        Ok(())
    }

    /// Pins a Plain Item into the next free Pin slot. Pinning a pinned Item does nothing.
    ///
    /// # Errors
    ///
    /// [`HistoryError::UnknownItem`] if there is no such Item, and
    /// [`HistoryError::NotPlain`] for Secret and Suspected Items.
    pub fn pin(&mut self, id: ItemId) -> Result<(), HistoryError> {
        let item = &self.items[self.index_of(id)?];
        if !item.sensitivity.may_pin() {
            return Err(HistoryError::NotPlain(id));
        }
        if !self.is_pinned(id) {
            self.pins.push(id);
        }
        Ok(())
    }

    /// Unpins an Item. The Pins after it move up one slot.
    ///
    /// # Errors
    ///
    /// [`HistoryError::UnknownItem`] if there is no such Item.
    pub fn unpin(&mut self, id: ItemId) -> Result<(), HistoryError> {
        self.index_of(id)?;
        self.pins.retain(|pinned| *pinned != id);
        self.evict();
        Ok(())
    }

    /// Whether the Item is pinned.
    #[must_use]
    pub fn is_pinned(&self, id: ItemId) -> bool {
        self.pins.contains(&id)
    }

    /// Pinned Items in Pin slot order.
    pub fn pins(&self) -> impl Iterator<Item = &Item> {
        self.pins.iter().filter_map(|id| self.get(*id))
    }

    /// The Item in Pin slot `slot`, counting from 1, as the global
    /// `Super+Alt+<slot>` shortcut pastes it (ADR 0004).
    #[must_use]
    pub fn pin_slot(&self, slot: usize) -> Option<&Item> {
        let id = self.pins.get(slot.checked_sub(1)?)?;
        self.get(*id)
    }

    /// Moves a pinned Item to Pin slot `slot`, counting from 1.
    ///
    /// # Errors
    ///
    /// [`HistoryError::NotPinned`] if the Item isn't pinned, and
    /// [`HistoryError::NoSuchPinSlot`] if `slot` is outside `1..=pins`.
    pub fn move_pin(&mut self, id: ItemId, slot: usize) -> Result<(), HistoryError> {
        let from = self
            .pins
            .iter()
            .position(|pinned| *pinned == id)
            .ok_or(HistoryError::NotPinned(id))?;
        if slot == 0 || slot > self.pins.len() {
            return Err(HistoryError::NoSuchPinSlot(slot));
        }
        let pinned = self.pins.remove(from);
        self.pins.insert(slot - 1, pinned);
        Ok(())
    }

    /// Changes an Item's Sensitivity by hand.
    ///
    /// Raising it to Suspected or Secret starts an Expiry from `now` and
    /// unpins the Item. Lowering it to Plain clears the Expiry.
    ///
    /// # Errors
    ///
    /// [`HistoryError::UnknownItem`] if there is no such Item.
    pub fn override_sensitivity(
        &mut self,
        id: ItemId,
        sensitivity: Sensitivity,
        now: Instant,
    ) -> Result<(), HistoryError> {
        let index = self.index_of(id)?;
        let expires_at = self.expiry_for(sensitivity, now);
        let item = &mut self.items[index];
        item.sensitivity = sensitivity;
        item.reason = Reason::Override;
        item.expires_at = expires_at;
        if !sensitivity.may_pin() {
            self.pins.retain(|pinned| *pinned != id);
        }
        self.evict();
        Ok(())
    }

    /// Erases every Item whose Expiry is at or before `now`, without a trace,
    /// and returns their ids.
    pub fn expire(&mut self, now: Instant) -> Vec<ItemId> {
        let mut expired = Vec::new();
        self.items.retain(|item| {
            let keep = item.expires_at.is_none_or(|at| at > now);
            if !keep {
                expired.push(item.id);
            }
            keep
        });
        expired
    }

    /// The earliest upcoming Expiry, so the daemon knows when to call [`History::expire`].
    #[must_use]
    pub fn next_expiry(&self) -> Option<Instant> {
        self.items.iter().filter_map(|item| item.expires_at).min()
    }

    /// Ids of the Items matching `query` (case-insensitive), newest first.
    ///
    /// Plain Items match on their text. Secret and Suspected Items match only
    /// on their description ("Looks like a GitHub token"), never on their
    /// content, so nobody can recover a secret by typing guesses. Images
    /// match on their MIME type. An empty query matches everything.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<ItemId> {
        let query = query.to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                query.is_empty() || searchable_text(item).to_lowercase().contains(&query)
            })
            .map(|item| item.id)
            .collect()
    }

    fn new_id(&mut self) -> ItemId {
        self.next_id += 1;
        ItemId(self.next_id)
    }

    fn index_of(&self, id: ItemId) -> Result<usize, HistoryError> {
        self.items
            .iter()
            .position(|item| item.id == id)
            .ok_or(HistoryError::UnknownItem(id))
    }

    fn expiry_for(&self, sensitivity: Sensitivity, now: Instant) -> Option<Instant> {
        match sensitivity {
            Sensitivity::Plain => None,
            Sensitivity::Suspected => Some(now + self.settings.suspected_expiry),
            Sensitivity::Secret => Some(now + self.settings.secret_expiry),
        }
    }

    /// Keeps the newest `max_plain_items` unpinned Plain Items and drops the rest.
    fn evict(&mut self) {
        let max = self.settings.max_plain_items;
        let pins = &self.pins;
        let mut kept = 0;
        self.items.retain(|item| {
            if item.sensitivity != Sensitivity::Plain || pins.contains(&item.id) {
                return true;
            }
            kept += 1;
            kept <= max
        });
    }
}

fn searchable_text(item: &Item) -> String {
    if item.sensitivity.is_masked() {
        return item.reason.description().to_owned();
    }
    match &item.kind {
        Kind::Text => String::from_utf8_lossy(item.content.expose()).into_owned(),
        Kind::RichText => {
            let text = item.plain_fallback.as_ref().unwrap_or(&item.content);
            String::from_utf8_lossy(text.expose()).into_owned()
        }
        Kind::Image { mime } => mime.clone(),
    }
}

#[cfg(test)]
mod tests;
