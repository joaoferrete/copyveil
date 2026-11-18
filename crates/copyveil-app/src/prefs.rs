//! The settings window's decisions, with no GTK in them.
//!
//! The daemon owns the settings and their limits (`Settings1`); this module
//! turns them into what the window shows and back.

use crate::N_;
use std::ops::RangeInclusive;

/// The limits from `Settings1.Limits`, as the window's spin rows use them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Seconds a Secret may live.
    pub secret_seconds: RangeInclusive<u32>,
    /// Minutes a Suspected Item may live.
    pub suspected_minutes: RangeInclusive<u32>,
    /// Unpinned Plain Items the History may keep.
    pub plain_items: RangeInclusive<u32>,
}

impl Limits {
    /// From `Settings1.Limits()`: seconds for both expiries, a count for items.
    #[must_use]
    pub fn from_dbus(limits: (u32, u32, u32, u32, u32, u32)) -> Self {
        let (secret_min, secret_max, suspected_min, suspected_max, items_min, items_max) = limits;
        Self {
            secret_seconds: secret_min..=secret_max,
            suspected_minutes: suspected_min.div_ceil(60)..=suspected_max / 60,
            plain_items: items_min..=items_max,
        }
    }
}

/// Seconds to whole minutes, for the Suspected expiry row.
#[must_use]
pub fn minutes(seconds: u32) -> u32 {
    seconds / 60
}

/// Whole minutes to seconds, for `Settings1.SuspectedExpiry`.
#[must_use]
pub fn seconds_from_minutes(minutes: u32) -> u32 {
    minutes.saturating_mul(60)
}

/// The rules that are always on, shown as locked rows so users see what
/// Copyveil guarantees whatever they set (ADR 0002).
pub const FIXED: [(&str, &str); 5] = [
    (
        N_!("Secrets never touch the disk"),
        N_!("Items from a password manager live in memory only"),
    ),
    (
        N_!("Suspected Items never touch the disk"),
        N_!("Anything that looks like a key or token lives in memory only"),
    ),
    (
        N_!("Saved History is encrypted"),
        N_!("With a key that lives only in your keyring"),
    ),
    (
        N_!("Search never looks inside secrets"),
        N_!("Secret and Suspected Items match only on their description"),
    ),
    (
        N_!("Pins are kept until you unpin them"),
        N_!("They don't count toward the History limit and never expire"),
    ),
];

/// What the confirmation says before memory-only erases the saved History.
pub const MEMORY_ONLY_CONFIRM: (&str, &str) = (
    N_!("Erase the saved History?"),
    N_!(
        "Copyveil will keep the History in memory only. The History saved on disk and its key in your keyring are erased now; what's in memory stays until you log out."
    ),
);

/// Where a Pin moves when nudged by `delta` slots, or `None` at the edges.
/// Slots count from 1.
#[must_use]
pub fn nudged_slot(slot: u32, delta: i32, pins: u32) -> Option<u32> {
    let target = slot.checked_add_signed(delta)?;
    (1..=pins)
        .contains(&target)
        .then_some(target)
        .filter(|t| *t != slot)
}

/// A GTK accelerator (`<Super><Shift>v`) as people read it (`Super+Shift+V`).
#[must_use]
pub fn accelerator_label(accelerator: &str) -> String {
    let mut parts = Vec::new();
    let mut rest = accelerator;
    while let Some(after) = rest.strip_prefix('<') {
        let Some((modifier, tail)) = after.split_once('>') else {
            break;
        };
        parts.push(match modifier.to_ascii_lowercase().as_str() {
            "primary" | "control" | "ctrl" => "Ctrl".to_owned(),
            "super" => "Super".to_owned(),
            "shift" => "Shift".to_owned(),
            "alt" => "Alt".to_owned(),
            other => other.to_owned(),
        });
        rest = tail;
    }
    if !rest.is_empty() {
        parts.push(if rest.chars().count() == 1 {
            rest.to_uppercase()
        } else {
            rest.to_owned()
        });
    }
    parts.join("+")
}

#[cfg(test)]
mod tests {
    use super::{
        FIXED, Limits, MEMORY_ONLY_CONFIRM, accelerator_label, minutes, nudged_slot,
        seconds_from_minutes,
    };

    #[test]
    fn limits_come_from_the_daemon() {
        let limits = Limits::from_dbus((10, 300, 60, 3600, 50, 5000));
        assert_eq!(limits.secret_seconds, 10..=300);
        assert_eq!(limits.suspected_minutes, 1..=60);
        assert_eq!(limits.plain_items, 50..=5000);
        // Minutes stay inside the limit in seconds when it isn't whole.
        let odd = Limits::from_dbus((10, 300, 90, 3630, 50, 5000));
        assert_eq!(odd.suspected_minutes, 2..=60);
    }

    #[test]
    fn suspected_expiry_is_shown_in_minutes() {
        assert_eq!(minutes(900), 15);
        assert_eq!(minutes(119), 1);
        assert_eq!(seconds_from_minutes(15), 900);
        assert_eq!(seconds_from_minutes(u32::MAX), u32::MAX);
    }

    #[test]
    fn pins_nudge_within_their_slots() {
        assert_eq!(nudged_slot(2, -1, 3), Some(1));
        assert_eq!(nudged_slot(2, 1, 3), Some(3));
        assert_eq!(nudged_slot(1, -1, 3), None);
        assert_eq!(nudged_slot(3, 1, 3), None);
        assert_eq!(nudged_slot(1, 0, 3), None);
    }

    #[test]
    fn accelerators_read_like_keys() {
        assert_eq!(accelerator_label("<Super><Shift>v"), "Super+Shift+V");
        assert_eq!(accelerator_label("<Super><Alt>1"), "Super+Alt+1");
        assert_eq!(accelerator_label("<Primary>comma"), "Ctrl+comma");
        assert_eq!(accelerator_label("<Control>v"), "Ctrl+V");
        assert_eq!(accelerator_label("F9"), "F9");
        assert_eq!(accelerator_label("<Super"), "<Super");
        assert_eq!(accelerator_label(""), "");
    }

    #[test]
    fn the_fixed_rules_and_the_confirmation_say_what_they_promise() {
        assert!(
            FIXED
                .iter()
                .any(|(title, _)| title.contains("never touch the disk"))
        );
        assert!(
            FIXED
                .iter()
                .any(|(title, _)| title.contains("Pins are kept"))
        );
        assert!(MEMORY_ONLY_CONFIRM.1.contains("keyring"));
    }
}
