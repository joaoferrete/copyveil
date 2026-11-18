//! What goes over D-Bus: Item summaries, kinds and Sensitivity names.
//!
//! A summary carries a preview of Plain Items only. Secret and Suspected
//! Items travel as their id, kind, Sensitivity and description; their content
//! crosses the bus only through an explicit `Content` call for one Item.

use copyveil_core::{History, Item, Kind, Sensitivity};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use zbus::zvariant::Type;

/// How many characters of a Plain Item's text the summary previews.
pub const PREVIEW_CHARS: usize = 300;

/// One Item as the app lists it. D-Bus signature `(tssssuttts)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ItemSummary {
    /// The Item id.
    pub id: u64,
    /// `text`, `rich-text` or `image`.
    pub kind: String,
    /// The MIME type of an image; empty otherwise.
    pub mime: String,
    /// `plain`, `suspected` or `secret`.
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

/// The name of a Sensitivity on the bus.
#[must_use]
pub const fn sensitivity_name(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Plain => "plain",
        Sensitivity::Suspected => "suspected",
        Sensitivity::Secret => "secret",
    }
}

/// Parses a Sensitivity name from the bus.
#[must_use]
pub fn parse_sensitivity(name: &str) -> Option<Sensitivity> {
    match name {
        "plain" => Some(Sensitivity::Plain),
        "suspected" => Some(Sensitivity::Suspected),
        "secret" => Some(Sensitivity::Secret),
        _ => None,
    }
}

/// The kind of a Capture, from the MIME type the capture source chose.
#[must_use]
pub fn kind_for_mime(mime: &str) -> Option<Kind> {
    let essence = mime.split(';').next().unwrap_or_default().trim();
    match essence {
        "text/plain" | "UTF8_STRING" | "STRING" | "TEXT" => Some(Kind::Text),
        "text/html" => Some(Kind::RichText),
        _ if essence.starts_with("image/") && essence.len() > "image/".len() => Some(Kind::Image {
            mime: essence.to_owned(),
        }),
        _ => None,
    }
}

/// Summaries of every Item, newest first.
#[must_use]
pub fn summaries(history: &History, now: Instant) -> Vec<ItemSummary> {
    let slots: Vec<_> = history.pins().map(Item::id).collect();
    history
        .items()
        .iter()
        .map(|item| {
            let pin_slot = slots
                .iter()
                .position(|id| *id == item.id())
                .map_or(0, |index| u32::try_from(index + 1).unwrap_or(u32::MAX));
            summary(item, pin_slot, now)
        })
        .collect()
}

fn summary(item: &Item, pin_slot: u32, now: Instant) -> ItemSummary {
    let (kind, mime) = match item.kind() {
        Kind::Text => ("text", ""),
        Kind::RichText => ("rich-text", ""),
        Kind::Image { mime } => ("image", mime.as_str()),
    };
    ItemSummary {
        id: item.id().value(),
        kind: kind.to_owned(),
        mime: mime.to_owned(),
        sensitivity: sensitivity_name(item.sensitivity()).to_owned(),
        reason: item.reason().description().to_owned(),
        pin_slot,
        age_ms: millis(now.saturating_duration_since(item.captured_at())),
        expires_in_ms: item
            .expires_at()
            .map_or(0, |at| millis(at.saturating_duration_since(now))),
        size: item.content().len() as u64,
        preview: preview(item),
    }
}

/// The start of a Plain Item's text. Masked Items and images get none.
fn preview(item: &Item) -> String {
    if item.sensitivity().is_masked() {
        return String::new();
    }
    let text = match item.kind() {
        Kind::Text => item.content(),
        Kind::RichText => item.plain_fallback().unwrap_or(item.content()),
        Kind::Image { .. } => return String::new(),
    };
    String::from_utf8_lossy(text.expose())
        .chars()
        .take(PREVIEW_CHARS)
        .collect()
}

fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{PREVIEW_CHARS, kind_for_mime, parse_sensitivity, sensitivity_name, summaries};
    use copyveil_core::{Capture, Content, History, Kind, Sensitivity, Settings};
    use std::time::{Duration, Instant};

    #[test]
    fn mime_types_map_to_kinds() {
        assert_eq!(kind_for_mime("text/plain"), Some(Kind::Text));
        assert_eq!(kind_for_mime("text/plain;charset=utf-8"), Some(Kind::Text));
        assert_eq!(kind_for_mime("UTF8_STRING"), Some(Kind::Text));
        assert_eq!(kind_for_mime("STRING"), Some(Kind::Text));
        assert_eq!(kind_for_mime("TEXT"), Some(Kind::Text));
        assert_eq!(kind_for_mime("text/html"), Some(Kind::RichText));
        assert_eq!(
            kind_for_mime("image/png"),
            Some(Kind::Image {
                mime: "image/png".into()
            })
        );
        assert_eq!(kind_for_mime("image/"), None);
        assert_eq!(kind_for_mime("text/uri-list"), None);
        assert_eq!(kind_for_mime(""), None);
    }

    #[test]
    fn sensitivity_names_round_trip() {
        for sensitivity in [
            Sensitivity::Plain,
            Sensitivity::Suspected,
            Sensitivity::Secret,
        ] {
            assert_eq!(
                parse_sensitivity(sensitivity_name(sensitivity)),
                Some(sensitivity)
            );
        }
        assert_eq!(parse_sensitivity("Secret"), None);
    }

    #[test]
    fn masked_items_have_no_preview() {
        let now = Instant::now();
        let mut history = History::new(Settings::default());
        let secret = history.capture(Capture::secret_text("COPYVEIL-MARKER-secret"), now);
        let suspected = history.capture(Capture::text("AKIAWBHK8C2D086BCGWN"), now);
        let later = now + Duration::from_secs(10);
        let list = summaries(&history, later);

        let [suspected_summary, secret_summary] = list.as_slice() else {
            panic!("expected two summaries");
        };
        assert_eq!(suspected_summary.id, suspected.value());
        assert_eq!(suspected_summary.sensitivity, "suspected");
        assert_eq!(suspected_summary.reason, "Looks like an AWS access key");
        assert_eq!(suspected_summary.preview, "");
        assert_eq!(secret_summary.id, secret.value());
        assert_eq!(secret_summary.sensitivity, "secret");
        assert_eq!(secret_summary.preview, "");
        assert_eq!(secret_summary.age_ms, 10_000);
        assert_eq!(secret_summary.expires_in_ms, 20_000);
        assert_eq!(secret_summary.size, 22);
        let debug = format!("{list:?}");
        assert!(!debug.contains("COPYVEIL-MARKER"));
        assert!(!debug.contains("AKIA"));
    }

    #[test]
    fn plain_items_preview_their_text() {
        let now = Instant::now();
        let mut history = History::new(Settings::default());
        history.capture(
            Capture {
                content: Content::new(vec![1, 2, 3]),
                kind: Kind::Image {
                    mime: "image/png".into(),
                },
                plain_fallback: None,
                password_manager_hint: false,
            },
            now,
        );
        history.capture(
            Capture {
                content: Content::from("<b>bold</b>"),
                kind: Kind::RichText,
                plain_fallback: Some(Content::from("bold")),
                password_manager_hint: false,
            },
            now,
        );
        history.capture(
            Capture {
                content: Content::from("<i>no fallback</i>"),
                kind: Kind::RichText,
                plain_fallback: None,
                password_manager_hint: false,
            },
            now,
        );
        let long = "é".repeat(PREVIEW_CHARS + 5);
        let pinned = history.capture(Capture::text(&long), now);
        history.pin(pinned).unwrap();

        let list = summaries(&history, now);
        assert_eq!(list[0].preview, "é".repeat(PREVIEW_CHARS));
        assert_eq!(list[0].kind, "text");
        assert_eq!(list[0].pin_slot, 1);
        assert_eq!(list[0].expires_in_ms, 0);
        assert_eq!(list[1].preview, "<i>no fallback</i>");
        assert_eq!(list[1].kind, "rich-text");
        assert_eq!(list[1].pin_slot, 0);
        assert_eq!(list[2].preview, "bold");
        assert_eq!(list[3].preview, "");
        assert_eq!(list[3].kind, "image");
        assert_eq!(list[3].mime, "image/png");
        assert_eq!(list[3].size, 3);
    }
}
