//! The onboarding's decisions, with no GTK in them.
//!
//! Four steps: what Copyveil is, where copies come from, where the History
//! is kept, and a live test. The live test can't be skipped the first
//! time: it proves capture, detection and masking work on this desktop.

use copyveil_dbus::{ItemSummary, kind, notice, sensitivity};
use std::path::PathBuf;

/// The onboarding's steps, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The promise, and what the three Sensitivities look like.
    Welcome,
    /// Where copies come from: the GNOME Shell extension, and Super+V.
    Capture,
    /// Where the History is kept: encrypted with the keyring, or in memory.
    Storage,
    /// Copy a synthetic secret and see it masked.
    LiveTest,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Self; 4] = [Self::Welcome, Self::Capture, Self::Storage, Self::LiveTest];

    /// Position from 0.
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap_or(0)
    }

    /// The step after this one, if any.
    #[must_use]
    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() + 1).copied()
    }

    /// The step before this one, if any.
    #[must_use]
    pub fn previous(self) -> Option<Self> {
        self.index()
            .checked_sub(1)
            .and_then(|i| Self::ALL.get(i).copied())
    }
}

/// Where the extension stands, from `org.gnome.Shell.Extensions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extension {
    /// GNOME Shell isn't reachable: another desktop, or no session.
    NoGnomeShell,
    /// GNOME Shell runs, but the extension isn't installed.
    NotInstalled,
    /// Installed but turned off (or failed to load).
    Off,
    /// Installed and running.
    On,
}

impl Extension {
    /// From `GetExtensionInfo`'s `state` (GNOME Shell's `ExtensionState`):
    /// 1 is active; an empty answer means not installed.
    #[must_use]
    pub fn from_state(state: Option<f64>) -> Self {
        match state {
            None => Self::NotInstalled,
            Some(state) if (state - 1.0).abs() < f64::EPSILON => Self::On,
            Some(_) => Self::Off,
        }
    }

    /// What the Capture step says.
    #[must_use]
    pub const fn describe(self) -> (&'static str, &'static str) {
        match self {
            Self::NoGnomeShell => (
                crate::N_!("Copyveil captures on GNOME for now"),
                crate::N_!(
                    "Support for KDE Plasma, COSMIC and wlroots desktops arrives in version 0.2."
                ),
            ),
            Self::NotInstalled => (
                crate::N_!("Install the GNOME Shell extension"),
                crate::N_!(
                    "GNOME only lets its own extensions watch the clipboard. Run “just ext-install” in the Copyveil source folder, then log out and back in."
                ),
            ),
            Self::Off => (
                crate::N_!("Turn on the GNOME Shell extension"),
                crate::N_!(
                    "It’s installed but not running. It passes each copy to Copyveil and stores nothing itself."
                ),
            ),
            Self::On => (
                crate::N_!("Copies reach Copyveil"),
                crate::N_!(
                    "The GNOME Shell extension is running. It passes each copy to Copyveil and stores nothing itself."
                ),
            ),
        }
    }
}

/// What the Storage step says, from `History1`'s `OnDisk` and
/// `StorageNotice` (`None` when the daemon isn't reachable).
#[must_use]
pub fn describe_storage(
    on_disk: Option<bool>,
    storage_notice: &str,
) -> (&'static str, &'static str) {
    match (on_disk, storage_notice) {
        (None, _) => (
            crate::N_!("Copyveil isn’t running"),
            crate::N_!("Start it with “systemctl --user enable --now copyveil-daemon”."),
        ),
        (Some(true), _) => (
            crate::N_!("Your History is encrypted"),
            crate::N_!(
                "Plain Items are saved encrypted with a key that lives only in your keyring. Secrets never touch the disk."
            ),
        ),
        (Some(false), notice::KEYRING_UNAVAILABLE) => (
            crate::N_!("No keyring found"),
            crate::N_!(
                "Copyveil keeps the History in memory only, so it clears when you log out. Install GNOME Keyring or KWallet to keep it."
            ),
        ),
        (Some(false), _) => (
            crate::N_!("Your History stays in memory"),
            crate::N_!(
                "Nothing is saved to disk, so the History clears when you log out. You can change this in Settings."
            ),
        ),
    }
}

/// A synthetic token in GitHub's format, different every time, so the test
/// always captures a new Item. `random` supplies the 36 characters.
#[must_use]
pub fn test_token(mut random: impl FnMut() -> u32) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let body: String = (0..36)
        .map(|_| {
            let index = usize::try_from(random()).unwrap_or(0) % ALPHABET.len();
            char::from(ALPHABET[index])
        })
        .collect();
    format!("ghp_{body}")
}

/// The test Item: one that wasn't in `before`, is Suspected, and was
/// flagged as a GitHub token. The app never sees masked content, so it
/// recognises the test by these, not by the value.
#[must_use]
pub fn find_test_item(before: &[u64], now: &[ItemSummary]) -> Option<u64> {
    find_test_items(before, now).first().copied()
}

/// Every Item that could be the test's, newest first. A copy can be
/// captured more than once (Mutter re-offers the clipboard), so cleaning
/// up removes them all.
#[must_use]
pub fn find_test_items(before: &[u64], now: &[ItemSummary]) -> Vec<u64> {
    now.iter()
        .filter(|item| {
            !before.contains(&item.id)
                && item.kind == kind::TEXT
                && item.sensitivity == sensitivity::SUSPECTED
                && item.reason == "Looks like a GitHub token"
        })
        .map(|item| item.id)
        .collect()
}

/// Whether the live test may be skipped: only after onboarding was
/// completed once.
#[must_use]
pub const fn live_test_skippable(completed_before: bool) -> bool {
    completed_before
}

/// Where the "onboarding done" mark lives: `$XDG_STATE_HOME/copyveil/onboarded`.
#[must_use]
pub fn done_marker(state_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    let state_home = state_home
        .filter(|dir| !dir.as_os_str().is_empty())
        .or_else(|| home.map(|home| home.join(".local/state")))?;
    Some(state_home.join("copyveil").join("onboarded"))
}

#[cfg(test)]
mod tests {
    use super::{
        Extension, Step, describe_storage, done_marker, find_test_item, find_test_items,
        live_test_skippable, test_token,
    };
    use copyveil_dbus::ItemSummary;
    use std::path::PathBuf;

    fn item(id: u64, sensitivity: &str, reason: &str) -> ItemSummary {
        ItemSummary {
            id,
            kind: "text".into(),
            mime: String::new(),
            sensitivity: sensitivity.into(),
            reason: reason.into(),
            pin_slot: 0,
            age_ms: 0,
            expires_in_ms: 0,
            size: 40,
            preview: String::new(),
        }
    }

    #[test]
    fn steps_go_in_order() {
        assert_eq!(Step::Welcome.next(), Some(Step::Capture));
        assert_eq!(Step::LiveTest.next(), None);
        assert_eq!(Step::Welcome.previous(), None);
        assert_eq!(Step::LiveTest.previous(), Some(Step::Storage));
        assert_eq!(Step::Storage.index(), 2);
    }

    #[test]
    fn extension_states() {
        assert_eq!(Extension::from_state(None), Extension::NotInstalled);
        assert_eq!(Extension::from_state(Some(1.0)), Extension::On);
        assert_eq!(Extension::from_state(Some(2.0)), Extension::Off);
        assert_eq!(Extension::from_state(Some(3.0)), Extension::Off);
        for state in [
            Extension::NoGnomeShell,
            Extension::NotInstalled,
            Extension::Off,
            Extension::On,
        ] {
            let (title, body) = state.describe();
            assert!(!title.is_empty() && body.ends_with('.'), "{state:?}");
        }
        assert!(
            Extension::NotInstalled
                .describe()
                .1
                .contains("just ext-install")
        );
    }

    #[test]
    fn storage_descriptions() {
        assert_eq!(describe_storage(None, "").0, "Copyveil isn’t running");
        assert_eq!(
            describe_storage(Some(true), "").0,
            "Your History is encrypted"
        );
        assert_eq!(
            describe_storage(Some(false), "keyring-unavailable").0,
            "No keyring found"
        );
        assert_eq!(
            describe_storage(Some(false), "").0,
            "Your History stays in memory"
        );
    }

    #[test]
    fn test_tokens_look_like_github_tokens_and_differ() {
        let mut n = 0;
        let token = test_token(|| {
            n += 7;
            n
        });
        assert!(token.starts_with("ghp_"));
        assert_eq!(token.len(), 40);
        assert!(token[4..].chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(token, test_token(|| 3));
        assert_eq!(
            copyveil_detect_check(&token),
            Some("Looks like a GitHub token")
        );
    }

    /// The token must be flagged by the real detector, or the test would never pass.
    fn copyveil_detect_check(token: &str) -> Option<&'static str> {
        copyveil_detect::detect(token, copyveil_detect::DetectorSet::all())
            .map(copyveil_detect::Detector::description)
    }

    #[test]
    fn the_test_item_is_new_suspected_and_a_github_token() {
        let before = [1, 2];
        let now = [
            item(3, "plain", ""),
            item(4, "suspected", "Looks like an AWS access key"),
            item(2, "suspected", "Looks like a GitHub token"),
            item(5, "suspected", "Looks like a GitHub token"),
        ];
        assert_eq!(find_test_item(&before, &now), Some(5));
        assert_eq!(find_test_item(&[1, 2, 5], &now), None);
        let twice = [
            item(6, "suspected", "Looks like a GitHub token"),
            now[3].clone(),
        ];
        assert_eq!(find_test_items(&before, &twice), [6, 5]);
    }

    #[test]
    fn the_live_test_is_mandatory_the_first_time() {
        assert!(!live_test_skippable(false));
        assert!(live_test_skippable(true));
    }

    #[test]
    fn the_done_marker_follows_xdg() {
        assert_eq!(
            done_marker(Some(PathBuf::from("/s")), Some(PathBuf::from("/h"))),
            Some(PathBuf::from("/s/copyveil/onboarded"))
        );
        assert_eq!(
            done_marker(Some(PathBuf::new()), Some(PathBuf::from("/h"))),
            Some(PathBuf::from("/h/.local/state/copyveil/onboarded"))
        );
        assert_eq!(done_marker(None, None), None);
    }
}
