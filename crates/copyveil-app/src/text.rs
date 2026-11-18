//! What the app says: ages, Expiry countdowns, the storage line and what a
//! screen reader hears. Pure functions over `copyveil-dbus` types.
//!
//! A screen reader never hears a masked value: it hears the Sensitivity, the
//! description and the time left (`docs/design/README.md`).

use copyveil_dbus::{ItemSummary, kind, notice, sensitivity};
use gettextrs::{gettext, ngettext};

/// Fills `{n}` in a translated template.
fn with_count(template: &str, count: u64) -> String {
    template.replace("{n}", &count.to_string())
}

const SECOND: u64 = 1_000;
const MINUTE: u64 = 60 * SECOND;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

/// How long ago an Item was copied: "just now", "4 min ago", "2 h ago", "3 days ago".
#[must_use]
pub fn age(ms: u64) -> String {
    match ms {
        ms if ms < MINUTE => gettext("just now"),
        ms if ms < HOUR => with_count(&gettext("{n} min ago"), ms / MINUTE),
        ms if ms < DAY => with_count(&gettext("{n} h ago"), ms / HOUR),
        ms if ms < 2 * DAY => gettext("yesterday"),
        ms => {
            let days = ms / DAY;
            with_count(
                &ngettext(
                    "{n} day ago",
                    "{n} days ago",
                    u32::try_from(days).unwrap_or(u32::MAX),
                ),
                days,
            )
        }
    }
}

/// Time left until an Expiry, rounded up so it never reads 0 before it
/// happens: "24 s", "15 min".
#[must_use]
pub fn time_left(ms: u64) -> String {
    if ms <= MINUTE {
        with_count(&gettext("{n} s"), ms.div_ceil(SECOND))
    } else {
        with_count(&gettext("{n} min"), ms.div_ceil(MINUTE))
    }
}

/// The same time left, spelled out for a screen reader: "24 seconds", "1 minute".
fn time_left_spoken(ms: u64) -> String {
    let as_u32 = |value: u64| u32::try_from(value).unwrap_or(u32::MAX);
    if ms <= MINUTE {
        let count = ms.div_ceil(SECOND);
        with_count(&ngettext("{n} second", "{n} seconds", as_u32(count)), count)
    } else {
        let count = ms.div_ceil(MINUTE);
        with_count(&ngettext("{n} minute", "{n} minutes", as_u32(count)), count)
    }
}

/// The popup's subtitle: where the History is kept. `None` means the
/// daemon isn't reachable.
#[must_use]
pub fn storage_title(on_disk: Option<bool>) -> String {
    match on_disk {
        None => gettext("Copyveil isn’t running"),
        Some(true) => gettext("Encrypted on disk"),
        Some(false) => gettext("Memory only"),
    }
}

/// What the banner under the header says about a storage notice, if anything.
#[must_use]
pub fn storage_notice(storage_notice: &str) -> Option<String> {
    match storage_notice {
        notice::KEYRING_UNAVAILABLE => Some(gettext(
            "No keyring found. History will clear when you log out.",
        )),
        notice::STORE_UNAVAILABLE => Some(gettext(
            "History can’t be saved, so it will clear when you log out.",
        )),
        notice::STORE_WIPED => Some(gettext(
            "The saved History couldn’t be opened with your keyring, so it was erased.",
        )),
        notice::ITEMS_UNREADABLE => Some(gettext(
            "Some saved Items couldn’t be read and were erased.",
        )),
        notice::SYNC_FAILED => Some(gettext(
            "Saving the History failed. Copyveil retries on the next copy.",
        )),
        _ => None,
    }
}

/// Where `query` occurs in `text`, ignoring case, as byte ranges that don't
/// overlap: what the search highlights in a Plain Item's preview.
#[must_use]
pub fn search_matches(text: &str, query: &str) -> Vec<std::ops::Range<usize>> {
    let needle: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let mut matched = 0;
        let mut j = i;
        while j < chars.len() && matched < needle.len() {
            let lower: Vec<char> = chars[j].1.to_lowercase().collect();
            if needle[matched..].starts_with(&lower) {
                matched += lower.len();
                j += 1;
            } else {
                break;
            }
        }
        if matched == needle.len() {
            let end = chars.get(j).map_or(text.len(), |(at, _)| *at);
            found.push(chars[i].0..end);
            i = j;
        } else {
            i += 1;
        }
    }
    found
}

/// How much of an Expiry is left, from 1 (just copied) to 0 (due), for the
/// `ExpiryRing`.
#[must_use]
pub fn expiry_fraction(remaining_ms: u64, total_ms: u64) -> f64 {
    if total_ms == 0 {
        return 0.0;
    }
    // Expiry times are at most an hour, so milliseconds fit a u32 exactly.
    let ms = |value: u64| f64::from(u32::try_from(value).unwrap_or(u32::MAX));
    let fraction = ms(remaining_ms.min(total_ms)) / ms(total_ms);
    fraction.clamp(0.0, 1.0)
}

/// An Item's reason, translated. The daemon sends it in English (it's also
/// what search matches); the app shows it in the user's language. Unknown
/// reasons pass through as they are.
#[must_use]
pub fn reason(english: &str) -> String {
    let marked = match english {
        "From password manager" => crate::N_!("From password manager"),
        "Changed by you" => crate::N_!("Changed by you"),
        "Looks like a GitHub token" => crate::N_!("Looks like a GitHub token"),
        "Looks like an AWS access key" => crate::N_!("Looks like an AWS access key"),
        "Looks like an AI provider API key" => crate::N_!("Looks like an AI provider API key"),
        "Looks like a JSON Web Token" => crate::N_!("Looks like a JSON Web Token"),
        "Looks like a private key" => crate::N_!("Looks like a private key"),
        "Looks like a payment card number" => crate::N_!("Looks like a payment card number"),
        "Looks like a random secret" => crate::N_!("Looks like a random secret"),
        other => return other.to_owned(),
    };
    gettext(marked)
}

/// What a screen reader hears for a row. Masked Items are announced by
/// Sensitivity, description and time left; their content never is.
#[must_use]
pub fn accessible_label(item: &ItemSummary) -> String {
    let mut parts = Vec::new();
    match item.sensitivity.as_str() {
        sensitivity::SECRET => parts.push(gettext("Secret, hidden")),
        sensitivity::SUSPECTED => parts.push(gettext("Suspected secret, hidden")),
        _ => parts.push(match item.kind.as_str() {
            kind::IMAGE => gettext("Image, {mime}").replace("{mime}", &item.mime),
            _ => item.preview.chars().take(120).collect(),
        }),
    }
    if !item.reason.is_empty() {
        parts.push(reason(&item.reason));
    }
    if item.expires_in_ms > 0 {
        parts.push(
            gettext("Expires in {time}").replace("{time}", &time_left_spoken(item.expires_in_ms)),
        );
    }
    if item.pin_slot > 0 {
        parts.push(with_count(
            &gettext("Pin slot {n}"),
            u64::from(item.pin_slot),
        ));
    }
    parts.join(". ") + "."
}

#[cfg(test)]
mod tests {
    use super::{
        accessible_label, age, expiry_fraction, search_matches, storage_notice, storage_title,
        time_left, time_left_spoken,
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

    fn none() -> Vec<std::ops::Range<usize>> {
        Vec::new()
    }

    #[test]
    fn search_matches_ignore_case_and_never_overlap() {
        assert_eq!(search_matches("git status", "STAT"), vec![(4..8)]);
        assert_eq!(search_matches("aaa", "aa"), vec![(0..2)]);
        assert_eq!(search_matches("Ação ação", "AÇÃO"), vec![0..6, 7..13]);
        assert_eq!(search_matches("docker", ""), none());
        assert_eq!(search_matches("docker", "x"), none());
        assert_eq!(search_matches("ab", "abc"), none());
    }

    #[test]
    fn the_expiry_ring_empties_as_time_passes() {
        assert!((expiry_fraction(30_000, 30_000) - 1.0).abs() < f64::EPSILON);
        assert!((expiry_fraction(15_000, 30_000) - 0.5).abs() < f64::EPSILON);
        assert!(expiry_fraction(0, 30_000).abs() < f64::EPSILON);
        assert!((expiry_fraction(40_000, 30_000) - 1.0).abs() < f64::EPSILON);
        assert!(expiry_fraction(10, 0).abs() < f64::EPSILON);
    }

    #[test]
    fn every_reason_the_daemon_sends_is_translatable() {
        for detector in copyveil_detect::Detector::ALL {
            let description = detector.description();
            assert!(
                REASONS.contains(&description),
                "{description} is missing from text::reason"
            );
        }
        assert_eq!(super::reason("Something new"), "Something new");
        assert_eq!(super::reason("Changed by you"), "Changed by you");
    }

    /// The reasons `text::reason` knows, kept in sync with it.
    const REASONS: [&str; 9] = [
        "From password manager",
        "Changed by you",
        "Looks like a GitHub token",
        "Looks like an AWS access key",
        "Looks like an AI provider API key",
        "Looks like a JSON Web Token",
        "Looks like a private key",
        "Looks like a payment card number",
        "Looks like a random secret",
    ];
}
