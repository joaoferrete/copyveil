//! Item content: the one type in Copyveil allowed to hold what the user copied.

use zeroize::Zeroizing;

/// The bytes of something the user copied.
///
/// `Content` implements neither `Debug` nor `Display`, so it cannot end up in a
/// log line, a panic message or an assertion by accident. Its memory is
/// overwritten with zeros when it is dropped. Reading it takes an explicit
/// call to [`Content::expose`], which is easy to find in review.
#[derive(Clone, PartialEq, Eq)]
pub struct Content(Zeroizing<Vec<u8>>);

impl Content {
    /// Takes ownership of `bytes`; they are zeroed when the `Content` is dropped.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// The raw bytes. Every call site is a place where content leaves the vault.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }

    /// Length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether there are no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for Content {
    fn from(text: &str) -> Self {
        Self::new(text.as_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::Content;
    use static_assertions::assert_not_impl_any;
    use std::fmt::{Debug, Display};

    assert_not_impl_any!(Content: Debug, Display);

    #[test]
    fn exposes_what_it_was_given() {
        let content = Content::from("hunter2");
        assert_eq!(content.expose(), b"hunter2");
        assert_eq!(content.len(), 7);
        assert!(!content.is_empty());
        assert!(Content::new(Vec::new()).is_empty());
    }

    #[test]
    fn equality_compares_bytes() {
        assert!(Content::from("a") == Content::from("a"));
        assert!(Content::from("a") != Content::from("b"));
    }
}
