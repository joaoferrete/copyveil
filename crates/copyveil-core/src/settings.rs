//! The settings that shape the History, and the limits they must stay within.
//!
//! The limits are domain rules, not UI hints: the daemon refuses any value
//! outside them, wherever it comes from (the settings file, D-Bus). The hard
//! floors in ADR 0002 aren't settings at all; nothing here can make a
//! Secret or Suspected Item persist.

use copyveil_detect::DetectorSet;
use std::fmt;
use std::ops::RangeInclusive;
use std::time::Duration;

/// How long a Secret may live before its Expiry: 10 s to 5 min.
pub const SECRET_EXPIRY: RangeInclusive<Duration> =
    Duration::from_secs(10)..=Duration::from_mins(5);
/// How long a Suspected Item may live before its Expiry: 1 min to 1 h.
pub const SUSPECTED_EXPIRY: RangeInclusive<Duration> =
    Duration::from_mins(1)..=Duration::from_hours(1);
/// How many unpinned Plain Items the History may keep: 50 to 5000. Pins
/// don't count; they are kept until unpinned.
pub const MAX_PLAIN_ITEMS: RangeInclusive<usize> = 50..=5000;

/// User settings that shape the History.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// How long a Secret lives before its Expiry.
    pub secret_expiry: Duration,
    /// How long a Suspected Item lives before its Expiry.
    pub suspected_expiry: Duration,
    /// How many unpinned Plain Items to keep. Older ones are removed first.
    pub max_plain_items: usize,
    /// Which detectors may flag an Item as Suspected.
    pub detectors: DetectorSet,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            secret_expiry: Duration::from_secs(30),
            suspected_expiry: Duration::from_mins(15),
            max_plain_items: 500,
            detectors: DetectorSet::all(),
        }
    }
}

impl Settings {
    /// Checks every value against its limit.
    ///
    /// # Errors
    ///
    /// The first [`SettingsError`] found.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if !SECRET_EXPIRY.contains(&self.secret_expiry) {
            return Err(SettingsError::SecretExpiry);
        }
        if !SUSPECTED_EXPIRY.contains(&self.suspected_expiry) {
            return Err(SettingsError::SuspectedExpiry);
        }
        if !MAX_PLAIN_ITEMS.contains(&self.max_plain_items) {
            return Err(SettingsError::MaxPlainItems);
        }
        Ok(())
    }
}

/// Which setting is outside its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsError {
    /// See [`SECRET_EXPIRY`].
    SecretExpiry,
    /// See [`SUSPECTED_EXPIRY`].
    SuspectedExpiry,
    /// See [`MAX_PLAIN_ITEMS`].
    MaxPlainItems,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::SecretExpiry => "Secret expiry must be between 10 seconds and 5 minutes",
            Self::SuspectedExpiry => "Suspected expiry must be between 1 minute and 1 hour",
            Self::MaxPlainItems => "the History must keep between 50 and 5000 Plain Items",
        })
    }
}

impl std::error::Error for SettingsError {}

#[cfg(test)]
mod tests {
    use super::{Settings, SettingsError};
    use std::time::Duration;

    const SECOND: Duration = Duration::from_secs(1);
    const NANO: Duration = Duration::from_nanos(1);

    fn with(change: impl FnOnce(&mut Settings)) -> Result<(), SettingsError> {
        let mut settings = Settings::default();
        change(&mut settings);
        settings.validate()
    }

    #[test]
    fn defaults_are_valid() {
        assert_eq!(Settings::default().validate(), Ok(()));
    }

    #[test]
    fn secret_expiry_limits_are_inclusive() {
        assert_eq!(with(|s| s.secret_expiry = 10 * SECOND), Ok(()));
        assert_eq!(with(|s| s.secret_expiry = 300 * SECOND), Ok(()));
        assert_eq!(
            with(|s| s.secret_expiry = (10 * SECOND).checked_sub(NANO).unwrap()),
            Err(SettingsError::SecretExpiry)
        );
        assert_eq!(
            with(|s| s.secret_expiry = 300 * SECOND + NANO),
            Err(SettingsError::SecretExpiry)
        );
    }

    #[test]
    fn suspected_expiry_limits_are_inclusive() {
        assert_eq!(with(|s| s.suspected_expiry = 60 * SECOND), Ok(()));
        assert_eq!(with(|s| s.suspected_expiry = 3600 * SECOND), Ok(()));
        assert_eq!(
            with(|s| s.suspected_expiry = (60 * SECOND).checked_sub(NANO).unwrap()),
            Err(SettingsError::SuspectedExpiry)
        );
        assert_eq!(
            with(|s| s.suspected_expiry = 3600 * SECOND + NANO),
            Err(SettingsError::SuspectedExpiry)
        );
    }

    #[test]
    fn plain_item_limits_are_inclusive() {
        assert_eq!(with(|s| s.max_plain_items = 50), Ok(()));
        assert_eq!(with(|s| s.max_plain_items = 5000), Ok(()));
        assert_eq!(
            with(|s| s.max_plain_items = 49),
            Err(SettingsError::MaxPlainItems)
        );
        assert_eq!(
            with(|s| s.max_plain_items = 5001),
            Err(SettingsError::MaxPlainItems)
        );
    }

    #[test]
    fn errors_name_the_limit() {
        assert_eq!(
            SettingsError::SecretExpiry.to_string(),
            "Secret expiry must be between 10 seconds and 5 minutes"
        );
        assert_eq!(
            SettingsError::SuspectedExpiry.to_string(),
            "Suspected expiry must be between 1 minute and 1 hour"
        );
        assert_eq!(
            SettingsError::MaxPlainItems.to_string(),
            "the History must keep between 50 and 5000 Plain Items"
        );
    }
}
