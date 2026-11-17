//! Random-looking tokens: long, mixed-class, high Shannon entropy.
//!
//! This is the catch-all for secrets with no recognisable prefix. It trades
//! some misses for few false positives: pure hex (commit hashes, UUIDs) and
//! embedded images are skipped, so hex-only secrets are left to the specific
//! detectors.

use regex::Regex;
use std::sync::LazyLock;

/// Shorter tokens rarely reach the entropy threshold by chance alone.
const MIN_LEN: usize = 24;
/// Longer runs are blobs (images, archives), not credentials.
const MAX_LEN: usize = 512;
/// Bits per character. Random base62 text of 24+ characters scores above 4.2;
/// identifiers and paths of the same length stay below.
const MIN_BITS_PER_CHAR: f64 = 4.0;
/// Lowercase, uppercase and digits. Random base62/base64 text of this length
/// almost always has all three; identifiers and URLs usually lack one.
const MIN_CHARACTER_CLASSES: usize = 3;
/// Tokens that are not secrets: npm/SRI integrity hashes, SSH public key blobs,
/// and paths (absolute paths, and URLs once `scheme:` is split off). Secrets in
/// a URL's query string are still checked, since `?`, `&` and `=` split tokens.
const PUBLIC_PREFIXES: [&str; 5] = ["sha256-", "sha384-", "sha512-", "AAAA", "/"];

#[expect(
    clippy::expect_used,
    reason = "the pattern is a constant; the tests exercise it"
)]
static DATA_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"data:[a-z]+/[a-z0-9.+-]+;base64,[A-Za-z0-9+/=]+")
        .expect("data URL pattern must compile")
});

pub(crate) fn contains_random_token(text: &str) -> bool {
    DATA_URL
        .replace_all(text, " ")
        .split(is_separator)
        .any(looks_random)
}

fn is_separator(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '"' | '\''
                | '`'
                | '='
                | ':'
                | ','
                | ';'
                | '&'
                | '?'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '<'
                | '>'
        )
}

fn looks_random(token: &str) -> bool {
    (MIN_LEN..=MAX_LEN).contains(&token.len())
        && token.chars().all(is_token_char)
        && character_classes(token) >= MIN_CHARACTER_CLASSES
        && !is_hex(token)
        && !has_public_prefix(token)
        && bits_per_char(token) >= MIN_BITS_PER_CHAR
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '-' | '.')
}

/// How many of lowercase, uppercase and digits appear.
fn character_classes(token: &str) -> usize {
    [
        token.bytes().any(|b| b.is_ascii_lowercase()),
        token.bytes().any(|b| b.is_ascii_uppercase()),
        token.bytes().any(|b| b.is_ascii_digit()),
    ]
    .into_iter()
    .filter(|&present| present)
    .count()
}

fn has_public_prefix(token: &str) -> bool {
    PUBLIC_PREFIXES
        .iter()
        .any(|prefix| token.starts_with(prefix))
}

/// Hex digits and dashes only: commit hashes, UUIDs, checksums.
fn is_hex(token: &str) -> bool {
    token.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// Shannon entropy in bits per byte. Callers pass tokens of at most `MAX_LEN` bytes.
fn bits_per_char(token: &str) -> f64 {
    let mut counts = [0_u32; 256];
    for byte in token.bytes() {
        counts[usize::from(byte)] += 1;
    }
    let len = f64::from(u32::try_from(token.len()).unwrap_or(u32::MAX));
    counts
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = f64::from(count) / len;
            -p * p.log2()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_LEN, MIN_LEN, bits_per_char, character_classes, contains_random_token,
        has_public_prefix, is_hex, is_separator, is_token_char, looks_random,
    };

    const RANDOM: &str = "45lCoG9ujKUwMLu7HjcUEGVgztPGdtAZ";

    #[test]
    fn entropy_of_known_strings() {
        assert!((bits_per_char("aaaa") - 0.0).abs() < 1e-9);
        assert!((bits_per_char("abab") - 1.0).abs() < 1e-9);
        assert!((bits_per_char("abcd") - 2.0).abs() < 1e-9);
        assert!((bits_per_char("aabc") - 1.5).abs() < 1e-9);
    }

    #[test]
    fn random_tokens_are_flagged() {
        assert!(looks_random(RANDOM));
        assert!(contains_random_token(&format!("API_KEY=\"{RANDOM}\"")));
    }

    #[test]
    fn length_bounds() {
        let token = "aB3dE5gH7jK9mN1pQ2sT4vW6yZ8bC0fL";
        assert!(looks_random(&token[..MIN_LEN]));
        assert!(!looks_random(&token[..MIN_LEN - 1]));
        let long: String = token.chars().cycle().take(MAX_LEN).collect();
        assert!(looks_random(&long));
        let too_long: String = token.chars().cycle().take(MAX_LEN + 1).collect();
        assert!(!looks_random(&too_long));
    }

    #[test]
    fn low_entropy_tokens_are_not_flagged() {
        assert!(!looks_random("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1"));
        assert!(!looks_random("SomeVeryLongIdentifierName1"));
    }

    #[test]
    fn character_classes_are_counted() {
        assert_eq!(character_classes("abc"), 1);
        assert_eq!(character_classes("ABC"), 1);
        assert_eq!(character_classes("123"), 1);
        assert_eq!(character_classes("aB"), 2);
        assert_eq!(character_classes("a1"), 2);
        assert_eq!(character_classes("A1"), 2);
        assert_eq!(character_classes("aB1"), 3);
        assert_eq!(character_classes("---"), 0);
    }

    #[test]
    fn tokens_missing_a_character_class_are_not_flagged() {
        assert!(!looks_random("qwhzmvkxprtyglbndscfjoeiuawq"));
        assert!(!looks_random("handleUserAuthenticationCallbackResponse"));
        assert!(!looks_random("k3j9x7q2m5w8z1v4b6n0p3r7t2y9"));
    }

    #[test]
    fn public_base64_is_skipped() {
        assert!(has_public_prefix(
            "sha512-z4PhNX7vuL3xVChQ1m2AB9Yg5AULVxXcg"
        ));
        assert!(has_public_prefix(
            "AAAAC3NzaC1lZDI1NTE5AAAAIGq5yPZ8ZQ2P8iHc0y5Jd1s"
        ));
        assert!(has_public_prefix(
            "//gitlab.gnome.org/GNOME/mutter/-/issues/1602"
        ));
        assert!(!has_public_prefix(RANDOM));
        assert!(contains_random_token(&format!(
            "https://example.com/cb?token={RANDOM}"
        )));
        assert!(!looks_random(
            "sha384-oqVuAfXRKap7fdgcCY5uykM6+R9GqQ8K/uxy9rx7HNQlGYl1kPzQho1wx4JwY8wC"
        ));
    }

    #[test]
    fn hex_is_skipped() {
        assert!(is_hex("3f34ce9a1b"));
        assert!(is_hex("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_hex("3f34ce9g"));
        assert!(!looks_random("0123456789abcdefABCDEF0123456789abcdefAB"));
    }

    #[test]
    fn token_characters() {
        for c in ['a', 'Z', '0', '+', '/', '_', '-', '.'] {
            assert!(is_token_char(c), "{c}");
        }
        for c in ['=', '!', ' ', 'é', '@'] {
            assert!(!is_token_char(c), "{c}");
        }
        assert!(!looks_random("45lCoG9ujKUwMLu7HjcU!EGVgztPGdtAZ"));
    }

    #[test]
    fn separators_split_tokens() {
        for c in [
            ' ', '\n', '\t', '"', '\'', '`', '=', ':', ',', ';', '&', '?', '(', ')', '[', ']', '{',
            '}', '<', '>',
        ] {
            assert!(is_separator(c), "{c:?}");
            assert!(contains_random_token(&format!("x{c}{RANDOM}{c}y")), "{c:?}");
        }
        for c in ['a', '-', '/', '.', '_'] {
            assert!(!is_separator(c), "{c:?}");
        }
    }

    #[test]
    fn embedded_images_are_skipped() {
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
        assert!(!contains_random_token(png));
        assert!(contains_random_token(&format!("{png} {RANDOM}")));
    }
}
