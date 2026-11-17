//! Detectors that are a single regular expression.
//!
//! `regex` runs in linear time, so no input can make a pattern slow.
//! Character classes are spelled out (`[0-9]`, not `\d`) to stay ASCII-only.

use regex::Regex;
use std::sync::LazyLock;

#[expect(
    clippy::expect_used,
    reason = "patterns are constants; the tests compile every one"
)]
fn compile(pattern: &str) -> Regex {
    Regex::new(pattern).expect("detector pattern must compile")
}

/// Classic tokens (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`) and fine-grained `github_pat_`.
pub(crate) static GITHUB_TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"\b(?:gh[pousr]_[A-Za-z0-9]{36,251}|github_pat_[A-Za-z0-9_]{22,244})\b")
});

/// Access key IDs for long-term (`AKIA`) and temporary (`ASIA`) credentials.
pub(crate) static AWS_ACCESS_KEY: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\b(?:AKIA|ASIA)[A-Z0-9]{16}\b"));

/// Anthropic keys (`sk-ant-`), project keys (`sk-proj-`) and legacy 48-character `sk-` keys.
pub(crate) static AI_PROVIDER_KEY: LazyLock<Regex> = LazyLock::new(|| {
    compile(r"\b(?:sk-ant-[A-Za-z0-9_-]{32,}|sk-proj-[A-Za-z0-9_-]{32,}|sk-[A-Za-z0-9]{48}\b)")
});

/// Header and payload are base64url JSON objects, so both start with `eyJ`.
pub(crate) static JWT: LazyLock<Regex> =
    LazyLock::new(|| compile(r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*"));

/// PEM private keys of any kind (RSA, EC, OPENSSH, ENCRYPTED…) and PGP private key blocks.
pub(crate) static PRIVATE_KEY: LazyLock<Regex> =
    LazyLock::new(|| compile(r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----"));

#[cfg(test)]
mod tests {
    use super::{AI_PROVIDER_KEY, AWS_ACCESS_KEY, GITHUB_TOKEN, JWT, PRIVATE_KEY};

    #[test]
    fn github_tokens_need_the_full_length() {
        assert!(GITHUB_TOKEN.is_match("ghp_GDuJSN7WM2qW0jG6gIUzNkxTczZ3jLNsAUQf"));
        assert!(!GITHUB_TOKEN.is_match("ghp_GDuJSN7WM2qW0jG6gIUzNkxTczZ3jLNsAUQ"));
        assert!(!GITHUB_TOKEN.is_match("ghx_GDuJSN7WM2qW0jG6gIUzNkxTczZ3jLNsAUQf"));
    }

    #[test]
    fn aws_keys_are_twenty_uppercase_characters() {
        assert!(AWS_ACCESS_KEY.is_match("AKIAWBHK8C2D086BCGWN"));
        assert!(!AWS_ACCESS_KEY.is_match("AKIAWBHK8C2D086BCGW"));
        assert!(!AWS_ACCESS_KEY.is_match("AKIAWBHK8C2D086BCGWNX"));
        assert!(!AWS_ACCESS_KEY.is_match("akiawbhk8c2d086bcgwn"));
    }

    #[test]
    fn legacy_openai_keys_are_exactly_48_characters() {
        let key = "sk-RTX5znn9Ct6hd9JS0AolxfEEKTPaauLVLv3j93txnaHCesyP";
        assert!(AI_PROVIDER_KEY.is_match(key));
        assert!(!AI_PROVIDER_KEY.is_match(&key[..key.len() - 1]));
        assert!(!AI_PROVIDER_KEY.is_match("sk-short"));
    }

    #[test]
    fn jwt_needs_three_parts() {
        let header = "eyJhbGciOiJIUzI1NiJ9";
        let payload = "eyJzdWIiOiJ0ZXN0In0";
        assert!(JWT.is_match(&format!("{header}.{payload}.c2lnbmF0dXJl")));
        assert!(JWT.is_match(&format!("{header}.{payload}.")));
        assert!(!JWT.is_match(&format!("{header}.c2lnbmF0dXJl")));
    }

    #[test]
    fn public_keys_are_not_private_keys() {
        assert!(PRIVATE_KEY.is_match("-----BEGIN PRIVATE KEY-----"));
        assert!(PRIVATE_KEY.is_match("-----BEGIN ENCRYPTED PRIVATE KEY-----"));
        assert!(!PRIVATE_KEY.is_match("-----BEGIN PUBLIC KEY-----"));
        assert!(!PRIVATE_KEY.is_match("-----BEGIN CERTIFICATE-----"));
    }
}
