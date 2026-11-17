//! Sensitivity: how an Item is stored, displayed and expired.
//!
//! The rules here are the hard floors from `docs/adr/0002-*`. Settings may tune
//! expiry times, but nothing may make a Secret or Suspected Item persist.

/// The classification of an Item that decides how it is stored, displayed and expired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sensitivity {
    /// No indication of being a secret.
    Plain,
    /// The content looks like a secret, but its source did not say so.
    Suspected,
    /// The source explicitly declared the content as secret.
    Secret,
}

impl Sensitivity {
    /// Whether an Item with this Sensitivity may be written to disk.
    #[must_use]
    pub const fn may_persist(self) -> bool {
        matches!(self, Self::Plain)
    }

    /// Whether an Item with this Sensitivity may be pinned.
    #[must_use]
    pub const fn may_pin(self) -> bool {
        matches!(self, Self::Plain)
    }

    /// Whether the UI must show this Item behind a Mask.
    #[must_use]
    pub const fn is_masked(self) -> bool {
        !matches!(self, Self::Plain)
    }

    /// Whether an Item with this Sensitivity has an Expiry.
    #[must_use]
    pub const fn expires(self) -> bool {
        !matches!(self, Self::Plain)
    }
}

#[cfg(test)]
mod tests {
    use super::Sensitivity::{Plain, Secret, Suspected};

    #[test]
    fn only_plain_items_reach_disk() {
        assert!(Plain.may_persist());
        assert!(!Suspected.may_persist());
        assert!(!Secret.may_persist());
    }

    #[test]
    fn only_plain_items_can_be_pinned() {
        assert!(Plain.may_pin());
        assert!(!Suspected.may_pin());
        assert!(!Secret.may_pin());
    }

    #[test]
    fn secret_and_suspected_are_masked() {
        assert!(!Plain.is_masked());
        assert!(Suspected.is_masked());
        assert!(Secret.is_masked());
    }

    #[test]
    fn secret_and_suspected_expire() {
        assert!(!Plain.expires());
        assert!(Suspected.expires());
        assert!(Secret.expires());
    }
}
