//! Items and the Captures they are made from.

use crate::{Content, Sensitivity};
use copyveil_detect::Detector;
use std::fmt;
use std::time::Instant;

/// How many characters [`Item::suspected_ends`] shows at each end.
pub const ENDS_CHARS: usize = 4;
/// The shortest value whose ends are shown: below this, eight characters
/// would be more than 40% of it.
pub const ENDS_MIN_CHARS: usize = 20;

/// Identifies an Item for as long as the daemon runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub(crate) u64);

impl ItemId {
    /// The id as a number, to send over D-Bus.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// An id received over D-Bus. History methods refuse ids they don't know.
    #[must_use]
    pub const fn from_value(value: u64) -> Self {
        Self(value)
    }
}

/// What kind of thing was copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Plain text (UTF-8).
    Text,
    /// Rich text (HTML). Detection and plain-text paste use the fallback.
    RichText,
    /// An image, with its MIME type (`image/png`, …).
    Image {
        /// The MIME type the image was offered as.
        mime: String,
    },
}

/// One clipboard change, as reported by a capture source.
pub struct Capture {
    /// What was copied.
    pub content: Content,
    /// The kind of content.
    pub kind: Kind,
    /// The plain-text version of rich text, when the source offered one.
    pub plain_fallback: Option<Content>,
    /// Whether the source flagged the content as secret
    /// (`x-kde-passwordManagerHint: secret`).
    pub password_manager_hint: bool,
}

impl Capture {
    /// A text Capture without a password manager hint.
    #[must_use]
    pub fn text(text: &str) -> Self {
        Self {
            content: Content::from(text),
            kind: Kind::Text,
            plain_fallback: None,
            password_manager_hint: false,
        }
    }

    /// A text Capture that a password manager flagged as secret.
    #[must_use]
    pub fn secret_text(text: &str) -> Self {
        Self {
            password_manager_hint: true,
            ..Self::text(text)
        }
    }
}

/// Why an Item has its Sensitivity. Shown to the user and used for search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// A password manager flagged it.
    PasswordManager,
    /// A detector flagged it.
    Detected(Detector),
    /// Nothing flagged it.
    Unflagged,
    /// The user changed the Sensitivity with an Override.
    Override,
}

impl Reason {
    /// The user-facing description. Search matches masked Items only against this.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::PasswordManager => "From password manager",
            Self::Detected(detector) => detector.description(),
            Self::Unflagged => "",
            Self::Override => "Changed by you",
        }
    }
}

/// A piece of content in the History.
pub struct Item {
    pub(crate) id: ItemId,
    pub(crate) content: Content,
    pub(crate) kind: Kind,
    pub(crate) plain_fallback: Option<Content>,
    pub(crate) sensitivity: Sensitivity,
    pub(crate) reason: Reason,
    pub(crate) captured_at: Instant,
    pub(crate) expires_at: Option<Instant>,
}

impl Item {
    /// The Item's id.
    #[must_use]
    pub fn id(&self) -> ItemId {
        self.id
    }

    /// What was copied. See [`Content::expose`].
    #[must_use]
    pub fn content(&self) -> &Content {
        &self.content
    }

    /// The kind of content.
    #[must_use]
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// The plain-text version of rich text, if any.
    #[must_use]
    pub fn plain_fallback(&self) -> Option<&Content> {
        self.plain_fallback.as_ref()
    }

    /// How the Item is stored, shown and expired.
    #[must_use]
    pub fn sensitivity(&self) -> Sensitivity {
        self.sensitivity
    }

    /// Why the Item has its Sensitivity.
    #[must_use]
    pub fn reason(&self) -> Reason {
        self.reason
    }

    /// The first and last [`ENDS_CHARS`] characters of a Suspected text
    /// Item whose value has at least [`ENDS_MIN_CHARS`], so the user can tell
    /// which token it is ("ghp_…a9f2"). `None` for everything else: Secret
    /// Items, Plain Items (shown in full), images, and short values, where
    /// eight characters would give away too much.
    #[must_use]
    pub fn suspected_ends(&self) -> Option<(String, String)> {
        if self.sensitivity != Sensitivity::Suspected {
            return None;
        }
        let text = match self.kind {
            Kind::Text => &self.content,
            Kind::RichText => self.plain_fallback.as_ref().unwrap_or(&self.content),
            Kind::Image { .. } => return None,
        };
        let text = String::from_utf8_lossy(text.expose());
        let chars: Vec<char> = text.trim().chars().collect();
        if chars.len() < ENDS_MIN_CHARS {
            return None;
        }
        let first = chars[..ENDS_CHARS].iter().collect();
        let last = chars[chars.len() - ENDS_CHARS..].iter().collect();
        Some((first, last))
    }

    /// When the content was last copied.
    #[must_use]
    pub fn captured_at(&self) -> Instant {
        self.captured_at
    }

    /// When the Item will be erased, for Secret and Suspected Items.
    #[must_use]
    pub fn expires_at(&self) -> Option<Instant> {
        self.expires_at
    }
}

/// Shows the id, kind and Sensitivity, never the content.
impl fmt::Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Item")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("sensitivity", &self.sensitivity)
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{Capture, Item, ItemId, Kind, Reason};
    use crate::{Content, Sensitivity};
    use copyveil_detect::Detector;
    use std::time::Instant;

    #[test]
    fn debug_never_shows_content() {
        let item = Item {
            id: ItemId(7),
            content: Content::from("COPYVEIL-MARKER-debug"),
            kind: Kind::Text,
            plain_fallback: Some(Content::from("COPYVEIL-MARKER-fallback")),
            sensitivity: Sensitivity::Secret,
            reason: Reason::PasswordManager,
            captured_at: Instant::now(),
            expires_at: None,
        };
        let debug = format!("{item:?}");
        assert!(!debug.contains("COPYVEIL-MARKER"));
        assert!(debug.contains("ItemId(7)"));
        assert!(debug.contains("Secret"));
    }

    #[test]
    fn reason_descriptions() {
        assert_eq!(
            Reason::PasswordManager.description(),
            "From password manager"
        );
        assert_eq!(
            Reason::Detected(Detector::Jwt).description(),
            "Looks like a JSON Web Token"
        );
        assert_eq!(Reason::Unflagged.description(), "");
        assert_eq!(Reason::Override.description(), "Changed by you");
    }

    fn item(content: &str, kind: Kind, sensitivity: Sensitivity) -> Item {
        Item {
            id: ItemId(1),
            content: Content::from(content),
            kind,
            plain_fallback: None,
            sensitivity,
            reason: Reason::Unflagged,
            captured_at: Instant::now(),
            expires_at: None,
        }
    }

    #[test]
    fn suspected_text_shows_four_characters_at_each_end() {
        let token = "ghp_TESTabcdefghijklmnopqrstuvwxyz0123a9f2";
        assert_eq!(
            item(token, Kind::Text, Sensitivity::Suspected).suspected_ends(),
            Some(("ghp_".to_owned(), "a9f2".to_owned()))
        );
        // Surrounding whitespace doesn't count, and characters aren't bytes.
        assert_eq!(
            item(
                "  ááááéééééééééééééóóóó \n",
                Kind::Text,
                Sensitivity::Suspected
            )
            .suspected_ends(),
            Some(("áááá".to_owned(), "óóóó".to_owned()))
        );
    }

    #[test]
    fn short_values_show_no_ends() {
        let nineteen = "a".repeat(19);
        let twenty = "b".repeat(20);
        assert_eq!(
            item(&nineteen, Kind::Text, Sensitivity::Suspected).suspected_ends(),
            None
        );
        assert_eq!(
            item(&twenty, Kind::Text, Sensitivity::Suspected).suspected_ends(),
            Some(("bbbb".to_owned(), "bbbb".to_owned()))
        );
    }

    #[test]
    fn only_suspected_text_shows_ends() {
        let token = "ghp_TESTabcdefghijklmnopqrstuvwxyz0123a9f2";
        assert_eq!(
            item(token, Kind::Text, Sensitivity::Secret).suspected_ends(),
            None
        );
        assert_eq!(
            item(token, Kind::Text, Sensitivity::Plain).suspected_ends(),
            None
        );
        let image = Kind::Image {
            mime: "image/png".into(),
        };
        assert_eq!(
            item(token, image, Sensitivity::Suspected).suspected_ends(),
            None
        );
    }

    #[test]
    fn rich_text_shows_the_ends_of_its_fallback() {
        let mut rich = item("<b>x</b>", Kind::RichText, Sensitivity::Suspected);
        rich.plain_fallback = Some(Content::from("AKIAWBHK8C2D086BCGWN-extra"));
        assert_eq!(
            rich.suspected_ends(),
            Some(("AKIA".to_owned(), "xtra".to_owned()))
        );
        rich.plain_fallback = None;
        assert_eq!(rich.suspected_ends(), None);
    }

    #[test]
    fn ids_round_trip_through_numbers() {
        assert_eq!(ItemId(42).value(), 42);
        assert_eq!(ItemId::from_value(42), ItemId(42));
    }

    #[test]
    fn capture_constructors() {
        let plain = Capture::text("x");
        assert!(!plain.password_manager_hint);
        assert_eq!(plain.kind, Kind::Text);
        assert!(plain.plain_fallback.is_none());
        assert!(Capture::secret_text("x").password_manager_hint);
    }
}
