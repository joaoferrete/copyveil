//! Detectors that decide whether text looks like a secret.
//!
//! A pure crate: no I/O, no async, no desktop. Everything here is a function
//! of its input, which keeps it easy to test, fuzz and mutation-test.

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
    /// Every detector, in the order the settings UI lists them.
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
}

#[cfg(test)]
mod tests {
    use super::Detector;
    use std::collections::HashSet;

    #[test]
    fn every_detector_has_a_distinct_description() {
        let descriptions: HashSet<_> = Detector::ALL.iter().map(|d| d.description()).collect();
        assert_eq!(descriptions.len(), Detector::ALL.len());
    }

    #[test]
    fn descriptions_start_with_looks_like() {
        for detector in Detector::ALL {
            assert!(
                detector.description().starts_with("Looks like "),
                "{detector:?}"
            );
        }
    }

    #[test]
    fn github_token_description() {
        assert_eq!(
            Detector::GitHubToken.description(),
            "Looks like a GitHub token"
        );
    }
}
