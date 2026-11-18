//! The popup's state, with no GTK in it.
//!
//! The widgets render a [`Popup`] and turn key presses into calls on it. All
//! the decisions live here, so they are unit tested: which Items show, which
//! one is selected, what <kbd>Ctrl</kbd>+number pastes, and what may be
//! pinned.

use copyveil_dbus::{ItemSummary, sensitivity};

/// The most rows that get a number for <kbd>Ctrl</kbd>+1…9.
pub const NUMBERED_ROWS: usize = 9;

/// Which Items the popup lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    /// Every Item, newest first.
    #[default]
    All,
    /// Pinned Items, in Pin slot order.
    Pinned,
}

/// Why an Item can't be pinned. The UI shows it as a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinRefusal {
    /// Secret Items are never pinned (ADR 0004).
    Secret,
    /// Suspected Items are never pinned, unless the user Overrides them to Plain.
    Suspected,
}

impl PinRefusal {
    /// What the toast says.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Secret => "Secrets can’t be pinned. They expire on their own.",
            Self::Suspected => {
                "This looks like a secret, so it can’t be pinned. Mark it “Not a secret” first."
            }
        }
    }
}

/// The popup: the History as last listed, the tab, the search and the selection.
#[derive(Debug, Default)]
pub struct Popup {
    items: Vec<ItemSummary>,
    /// Ids matching the search, newest first; `None` when not searching.
    matches: Option<Vec<u64>>,
    tab: Tab,
    /// Id of the selected Item, kept across refreshes.
    selected: Option<u64>,
}

impl Popup {
    /// Replaces the listed Items (after `History1.Changed`). The selection
    /// stays on the same Item when it's still there, and otherwise moves to
    /// the first row.
    pub fn set_items(&mut self, items: Vec<ItemSummary>) {
        self.items = items;
        self.keep_selection_visible();
    }

    /// Sets the search results from `History1.Search`, or `None` to stop
    /// searching. The first match is selected.
    pub fn set_matches(&mut self, matches: Option<Vec<u64>>) {
        self.matches = matches;
        self.selected = self.visible().first().map(|item| item.id);
    }

    /// Updates the search results after the History changed, keeping the
    /// selection on the same Item when it still matches.
    pub fn set_matches_keeping_selection(&mut self, matches: Option<Vec<u64>>) {
        self.matches = matches;
        self.keep_selection_visible();
    }

    /// The current tab.
    #[must_use]
    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// Switches tab and selects its first row.
    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.selected = self.visible().first().map(|item| item.id);
    }

    /// The rows to show, in order.
    #[must_use]
    pub fn visible(&self) -> Vec<&ItemSummary> {
        let mut rows: Vec<&ItemSummary> = self
            .items
            .iter()
            .filter(|item| self.tab == Tab::All || item.pin_slot > 0)
            .filter(|item| {
                self.matches
                    .as_ref()
                    .is_none_or(|matches| matches.contains(&item.id))
            })
            .collect();
        if self.tab == Tab::Pinned {
            rows.sort_by_key(|item| item.pin_slot);
        }
        rows
    }

    /// The selected row's Item.
    #[must_use]
    pub fn selected(&self) -> Option<&ItemSummary> {
        let id = self.selected?;
        self.visible().into_iter().find(|item| item.id == id)
    }

    /// The selected row's position in [`Popup::visible`].
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected?;
        self.visible().iter().position(|item| item.id == id)
    }

    /// Moves the selection by `delta` rows (arrow keys), stopping at the ends.
    pub fn move_selection(&mut self, delta: isize) {
        let rows = self.visible();
        let Some(last) = rows.len().checked_sub(1) else {
            self.selected = None;
            return;
        };
        let current = self.selected_index().unwrap_or(0);
        let target = current.saturating_add_signed(delta).min(last);
        self.selected = Some(rows[target].id);
    }

    /// Selects the Item with this id, if it's showing (a click).
    pub fn select(&mut self, id: u64) {
        if self.visible().iter().any(|item| item.id == id) {
            self.selected = Some(id);
        }
    }

    /// The Item <kbd>Ctrl</kbd>+`number` pastes: the row with that number,
    /// counting from 1. In the popup this is a History position you can see
    /// (ADR 0004).
    #[must_use]
    pub fn numbered(&self, number: usize) -> Option<&ItemSummary> {
        if !(1..=NUMBERED_ROWS).contains(&number) {
            return None;
        }
        self.visible().get(number - 1).copied()
    }

    /// Whether the selected Item may be pinned, and why not.
    ///
    /// # Errors
    ///
    /// The [`PinRefusal`] to show when the selected Item isn't Plain.
    pub fn may_pin(item: &ItemSummary) -> Result<(), PinRefusal> {
        match item.sensitivity.as_str() {
            sensitivity::SECRET => Err(PinRefusal::Secret),
            sensitivity::SUSPECTED => Err(PinRefusal::Suspected),
            _ => Ok(()),
        }
    }

    /// Whether the Item offers "Not a secret": only Suspected Items do. A
    /// Secret was declared by its source, so the popup doesn't second-guess it.
    #[must_use]
    pub fn may_unflag(item: &ItemSummary) -> bool {
        item.sensitivity == sensitivity::SUSPECTED
    }

    fn keep_selection_visible(&mut self) {
        let rows = self.visible();
        let still_there = self
            .selected
            .is_some_and(|id| rows.iter().any(|item| item.id == id));
        if !still_there {
            self.selected = rows.first().map(|item| item.id);
        }
    }
}

/// The number shown on a row for <kbd>Ctrl</kbd>+number, from its position.
#[must_use]
pub fn row_number(index: usize) -> Option<usize> {
    (index < NUMBERED_ROWS).then_some(index + 1)
}

#[cfg(test)]
mod tests;

/// The detector that flagged an Item, by matching its reason against
/// `Settings1.Detectors()` (`(name, title, description)`). Returns the
/// detector's name and title, for "Stop flagging GitHub tokens".
#[must_use]
pub fn detector_for<'a>(
    reason: &str,
    detectors: &'a [(String, String, String)],
) -> Option<(&'a str, &'a str)> {
    detectors
        .iter()
        .find(|(_, _, description)| description == reason)
        .map(|(name, title, _)| (name.as_str(), title.as_str()))
}
