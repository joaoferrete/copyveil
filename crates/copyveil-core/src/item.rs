//! Items and the Captures they are made from.

use crate::{Content, Sensitivity};
use copyveil_detect::Detector;
use std::fmt;
use std::time::Instant;

/// Identifies an Item for as long as the daemon runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId(pub(crate) u64);

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

    #[test]
    fn capture_constructors() {
        let plain = Capture::text("x");
        assert!(!plain.password_manager_hint);
        assert_eq!(plain.kind, Kind::Text);
        assert!(plain.plain_fallback.is_none());
        assert!(Capture::secret_text("x").password_manager_hint);
    }
}
