//! What the app says: ages, Expiry countdowns, the storage line and what a
//! screen reader hears. Pure functions over `copyveil-dbus` types.
//!
//! A screen reader never hears a masked value: it hears the Sensitivity, the
//! description and the time left (`docs/design/README.md`).

use copyveil_dbus::{ItemSummary, kind, notice, sensitivity};

const SECOND: u64 = 1_000;
const MINUTE: u64 = 60 * SECOND;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// How long ago an Item was copied: "just now", "4 min ago", "2 h ago", "3 days ago".
#[must_use]
pub fn age(ms: u64) -> String {
    match ms {
        ms if ms < MINUTE => "just now".to_owned(),
        ms if ms < HOUR => format!("{} min ago", ms / MINUTE),
        ms if ms < DAY => format!("{} h ago", ms / HOUR),
        ms if ms < 2 * DAY => "yesterday".to_owned(),
        ms => format!("{} days ago", ms / DAY),
    }
}

/// Time left until an Expiry, rounded up so it never reads 0 before it
/// happens: "24 s", "15 min".
#[must_use]
pub fn time_left(ms: u64) -> String {
    if ms <= MINUTE {
        format!("{} s", ms.div_ceil(SECOND))
    } else {
        format!("{} min", ms.div_ceil(MINUTE))
    }
}

/// The same time left, spelled out for a screen reader: "24 seconds", "1 minute".
fn time_left_spoken(ms: u64) -> String {
    let (count, unit) = if ms <= MINUTE {
        (ms.div_ceil(SECOND), "second")
    } else {
        (ms.div_ceil(MINUTE), "minute")
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural}")
}

/// The popup's subtitle: where the History is kept. `None` means the
/// daemon isn't reachable.
#[must_use]
pub fn storage_title(on_disk: Option<bool>) -> &'static str {
    match on_disk {
        None => "Copyveil isn’t running",
        Some(true) => "Encrypted on disk",
        Some(false) => "Memory only",
    }
}

/// What the banner under the header says about a storage notice, if anything.
#[must_use]
pub fn storage_notice(storage_notice: &str) -> Option<&'static str> {
    match storage_notice {
        notice::KEYRING_UNAVAILABLE => {
            Some("No keyring found. History will clear when you log out.")
        }
        notice::STORE_UNAVAILABLE => {
            Some("History can’t be saved, so it will clear when you log out.")
        }
        notice::STORE_WIPED => {
            Some("The saved History couldn’t be opened with your keyring, so it was erased.")
        }
        notice::ITEMS_UNREADABLE => Some("Some saved Items couldn’t be read and were erased."),
        notice::SYNC_FAILED => {
            Some("Saving the History failed. Copyveil retries on the next copy.")
        }
        _ => None,
    }
}

/// What a screen reader hears for a row. Masked Items are announced by
/// Sensitivity, description and time left; their content never is.
#[must_use]
pub fn accessible_label(item: &ItemSummary) -> String {
    let mut parts = Vec::new();
    match item.sensitivity.as_str() {
        sensitivity::SECRET => parts.push("Secret, hidden".to_owned()),
        sensitivity::SUSPECTED => parts.push("Suspected secret, hidden".to_owned()),
        _ => parts.push(match item.kind.as_str() {
            kind::IMAGE => format!("Image, {}", item.mime),
            _ => item.preview.chars().take(120).collect(),
        }),
    }
    if !item.reason.is_empty() {
        parts.push(item.reason.clone());
    }
    if item.expires_in_ms > 0 {
        parts.push(format!(
            "Expires in {}",
            time_left_spoken(item.expires_in_ms)
        ));
    }
    if item.pin_slot > 0 {
        parts.push(format!("Pin slot {}", item.pin_slot));
    }
    parts.join(". ") + "."
}

#[cfg(test)]
mod tests {
    use super::{
        accessible_label, age, storage_notice, storage_title, time_left, time_left_spoken,
    };
    use copyveil_dbus::ItemSummary;

    fn summary(sensitivity: &str) -> ItemSummary {
        ItemSummary {
            id: 1,
            kind: "text".into(),
            mime: String::new(),
            sensitivity: sensitivity.into(),
            reason: String::new(),
            pin_slot: 0,
            age_ms: 0,
            expires_in_ms: 0,
            size: 4,
            preview: String::new(),
        }
    }

    #[test]
    fn ages() {
        assert_eq!(age(0), "just now");
        assert_eq!(age(59_999), "just now");
        assert_eq!(age(60_000), "1 min ago");
        assert_eq!(age(3_599_999), "59 min ago");
        assert_eq!(age(3_600_000), "1 h ago");
        assert_eq!(age(86_399_999), "23 h ago");
        assert_eq!(age(86_400_000), "yesterday");
        assert_eq!(age(172_800_000), "2 days ago");
    }

    #[test]
    fn time_left_rounds_up() {
        assert_eq!(time_left(1), "1 s");
        assert_eq!(time_left(23_001), "24 s");
        assert_eq!(time_left(60_000), "60 s");
        assert_eq!(time_left(60_001), "2 min");
        assert_eq!(time_left(900_000), "15 min");
        assert_eq!(time_left_spoken(1_000), "1 second");
        assert_eq!(time_left_spoken(24_000), "24 seconds");
        assert_eq!(time_left_spoken(60_001), "2 minutes");
        assert_eq!(time_left_spoken(60_000), "60 seconds");
        assert_eq!(time_left_spoken(120_000), "2 minutes");
    }

    #[test]
    fn storage_titles_and_notices() {
        assert_eq!(storage_title(None), "Copyveil isn’t running");
        assert_eq!(storage_title(Some(true)), "Encrypted on disk");
        assert_eq!(storage_title(Some(false)), "Memory only");
        assert_eq!(storage_notice(""), None);
        assert_eq!(storage_notice("unknown"), None);
        for name in [
            "keyring-unavailable",
            "store-unavailable",
            "store-wiped",
            "items-unreadable",
            "sync-failed",
        ] {
            let message = storage_notice(name).unwrap();
            assert!(message.ends_with('.'), "{name}");
        }
        assert!(
            storage_notice("keyring-unavailable")
                .unwrap()
                .contains("log out")
        );
        assert!(storage_notice("store-wiped").unwrap().contains("erased"));
    }

    #[test]
    fn masked_items_are_announced_without_content() {
        let secret = ItemSummary {
            reason: "From password manager".into(),
            expires_in_ms: 23_500,
            ..summary("secret")
        };
        assert_eq!(
            accessible_label(&secret),
            "Secret, hidden. From password manager. Expires in 24 seconds."
        );
        let suspected = ItemSummary {
            reason: "Looks like a GitHub token".into(),
            expires_in_ms: 840_000,
            ..summary("suspected")
        };
        assert_eq!(
            accessible_label(&suspected),
            "Suspected secret, hidden. Looks like a GitHub token. Expires in 14 minutes."
        );
    }

    #[test]
    fn plain_items_are_announced_by_their_preview() {
        let plain = ItemSummary {
            preview: "git status".into(),
            pin_slot: 2,
            ..summary("plain")
        };
        assert_eq!(accessible_label(&plain), "git status. Pin slot 2.");

        let long = ItemSummary {
            preview: "a".repeat(200),
            ..summary("plain")
        };
        assert_eq!(accessible_label(&long), format!("{}.", "a".repeat(120)));

        let image = ItemSummary {
            kind: "image".into(),
            mime: "image/png".into(),
            ..summary("plain")
        };
        assert_eq!(accessible_label(&image), "Image, image/png.");
    }
}
