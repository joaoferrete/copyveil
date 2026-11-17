//! Detectors that decide whether text looks like a secret.
//!
//! A pure crate: no I/O, no async, no desktop. Everything here is a function
//! of its input, which keeps it easy to test, fuzz and mutation-test.
//!
//! An Item is Suspected when any enabled detector finds a secret anywhere in
//! its text, so a pasted `.env` line is flagged as well as a bare token.
//!
//! ```
//! use copyveil_detect::{detect, Detector, DetectorSet};
//!
//! let text = "export AWS_ACCESS_KEY_ID=AKIAWBHK8C2D086BCGWN";
//! assert_eq!(detect(text, DetectorSet::all()), Some(Detector::AwsAccessKey));
//! assert_eq!(detect("git rebase -i HEAD~3", DetectorSet::all()), None);
//! ```

mod card;
mod entropy;
mod patterns;

/// A detector that can flag text as Suspected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Detector {
    /// GitHub personal access and OAuth tokens (`ghp_`, `gho_`, `github_pat_`).
    GitHubToken,
    /// AWS access key IDs (`AKIA…`).
    AwsAccessKey,
    /// API keys from AI providers (`sk-ant-`, `sk-proj-`).
    AiProviderKey,
    /// JSON Web Tokens.
    Jwt,
    /// PEM-encoded private keys.
    PrivateKey,
    /// Payment card numbers that pass the Luhn check.
    PaymentCard,
    /// Random-looking strings with high Shannon entropy.
    HighEntropy,
}

impl Detector {
    /// Every detector, in the order the settings UI lists them and [`detect`] tries them.
    ///
    /// Specific detectors come before [`Detector::HighEntropy`], so a known token
    /// format is reported as itself rather than as random text.
    pub const ALL: [Self; 7] = [
        Self::GitHubToken,
        Self::AwsAccessKey,
        Self::AiProviderKey,
        Self::Jwt,
        Self::PrivateKey,
        Self::PaymentCard,
        Self::HighEntropy,
    ];

    /// The user-facing description shown next to a Suspected Item.
    ///
    /// Search matches against this text, never against the Item's content.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::GitHubToken => "Looks like a GitHub token",
            Self::AwsAccessKey => "Looks like an AWS access key",
            Self::AiProviderKey => "Looks like an AI provider API key",
            Self::Jwt => "Looks like a JSON Web Token",
            Self::PrivateKey => "Looks like a private key",
            Self::PaymentCard => "Looks like a payment card number",
            Self::HighEntropy => "Looks like a random secret",
        }
    }

    /// Whether this detector finds a secret anywhere in `text`.
    #[must_use]
    pub fn matches(self, text: &str) -> bool {
        match self {
            Self::GitHubToken => patterns::GITHUB_TOKEN.is_match(text),
            Self::AwsAccessKey => patterns::AWS_ACCESS_KEY.is_match(text),
            Self::AiProviderKey => patterns::AI_PROVIDER_KEY.is_match(text),
            Self::Jwt => patterns::JWT.is_match(text),
            Self::PrivateKey => patterns::PRIVATE_KEY.is_match(text),
            Self::PaymentCard => card::contains_card_number(text),
            Self::HighEntropy => entropy::contains_random_token(text),
        }
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// The set of detectors the user has enabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectorSet(u8);

impl DetectorSet {
    /// Every detector enabled. This is the default.
    #[must_use]
    pub const fn all() -> Self {
        // `Detector::bit` gives each detector `1 << discriminant`, and the
        // discriminants are 0..ALL.len(), so the full set is the low bits.
        Self((1 << Detector::ALL.len()) - 1)
    }

    /// No detector enabled.
    #[must_use]
    pub const fn none() -> Self {
        Self(0)
    }

    /// This set with `detector` enabled.
    #[must_use]
    pub const fn with(self, detector: Detector) -> Self {
        Self(self.0 | detector.bit())
    }

    /// This set with `detector` disabled.
    #[must_use]
    pub const fn without(self, detector: Detector) -> Self {
        Self(self.0 & !detector.bit())
    }

    /// Whether `detector` is enabled.
    #[must_use]
    pub const fn contains(self, detector: Detector) -> bool {
        self.0 & detector.bit() != 0
    }
}

impl Default for DetectorSet {
    fn default() -> Self {
        Self::all()
    }
}

/// The first enabled detector, in [`Detector::ALL`] order, that finds a secret in `text`.
#[must_use]
pub fn detect(text: &str, enabled: DetectorSet) -> Option<Detector> {
    Detector::ALL
        .into_iter()
        .filter(|detector| enabled.contains(*detector))
        .find(|detector| detector.matches(text))
}

#[cfg(test)]
mod tests {
    use super::{Detector, DetectorSet, detect};
    use std::collections::HashSet;

    const AWS_KEY: &str = "AKIAWBHK8C2D086BCGWN";

    #[test]
    fn every_detector_has_a_distinct_description() {
        let descriptions: HashSet<_> = Detector::ALL.iter().map(|d| d.description()).collect();
        assert_eq!(descriptions.len(), Detector::ALL.len());
    }

    #[test]
    fn descriptions_name_what_was_found() {
        let expected = [
            "Looks like a GitHub token",
            "Looks like an AWS access key",
            "Looks like an AI provider API key",
            "Looks like a JSON Web Token",
            "Looks like a private key",
            "Looks like a payment card number",
            "Looks like a random secret",
        ];
        let actual: Vec<_> = Detector::ALL.iter().map(|d| d.description()).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_detector_has_its_own_bit() {
        let bits: HashSet<_> = Detector::ALL.iter().map(|d| d.bit()).collect();
        assert_eq!(bits.len(), Detector::ALL.len());
        assert_eq!(Detector::GitHubToken.bit(), 1);
        assert_eq!(Detector::HighEntropy.bit(), 1 << 6);
    }

    #[test]
    fn all_contains_every_detector_and_none_contains_none() {
        for detector in Detector::ALL {
            assert!(DetectorSet::all().contains(detector), "{detector:?}");
            assert!(!DetectorSet::none().contains(detector), "{detector:?}");
        }
    }

    #[test]
    fn default_is_all() {
        assert_eq!(DetectorSet::default(), DetectorSet::all());
    }

    #[test]
    fn with_and_without_toggle_only_one_detector() {
        let set = DetectorSet::all().without(Detector::Jwt);
        assert!(!set.contains(Detector::Jwt));
        assert!(set.contains(Detector::PrivateKey));
        assert_eq!(set.with(Detector::Jwt), DetectorSet::all());
        assert_eq!(
            DetectorSet::all().with(Detector::Jwt),
            DetectorSet::all(),
            "enabling an enabled detector keeps it enabled"
        );
        assert_eq!(
            DetectorSet::none().without(Detector::Jwt),
            DetectorSet::none(),
            "disabling a disabled detector keeps it disabled"
        );

        let only_jwt = DetectorSet::none().with(Detector::Jwt);
        assert!(only_jwt.contains(Detector::Jwt));
        assert!(!only_jwt.contains(Detector::GitHubToken));
        assert_eq!(only_jwt.without(Detector::Jwt), DetectorSet::none());
    }

    #[test]
    fn detect_finds_secrets_inside_longer_text() {
        let text = format!("export AWS_ACCESS_KEY_ID={AWS_KEY}");
        assert_eq!(
            detect(&text, DetectorSet::all()),
            Some(Detector::AwsAccessKey)
        );
    }

    #[test]
    fn detect_skips_disabled_detectors() {
        let set = DetectorSet::all().without(Detector::AwsAccessKey);
        assert_ne!(detect(AWS_KEY, set), Some(Detector::AwsAccessKey));
        assert_eq!(detect(AWS_KEY, DetectorSet::none()), None);
    }

    #[test]
    fn specific_detectors_win_over_high_entropy() {
        let token = "ghp_GDuJSN7WM2qW0jG6gIUzNkxTczZ3jLNsAUQf";
        assert!(Detector::HighEntropy.matches(token));
        assert_eq!(
            detect(token, DetectorSet::all()),
            Some(Detector::GitHubToken)
        );
    }

    #[test]
    fn plain_text_is_not_flagged() {
        assert_eq!(detect("", DetectorSet::all()), None);
        assert_eq!(detect("git rebase -i HEAD~3", DetectorSet::all()), None);
    }
}
