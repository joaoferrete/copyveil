//! Payment card numbers: 13 to 19 digits, optionally grouped with spaces or
//! dashes, with a known issuer prefix and a valid Luhn check digit.

use regex::Regex;
use std::sync::LazyLock;

#[expect(
    clippy::expect_used,
    reason = "the pattern is a constant; the tests exercise it"
)]
static CANDIDATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[0-9](?:[ -]?[0-9]){12,18}\b").expect("card pattern must compile")
});

/// Luhn's doubled digit, with the "subtract 9" step folded into the table.
const DOUBLED: [u32; 10] = [0, 2, 4, 6, 8, 1, 3, 5, 7, 9];

pub(crate) fn contains_card_number(text: &str) -> bool {
    CANDIDATE
        .find_iter(text)
        .any(|candidate| is_card_number(candidate.as_str()))
}

fn is_card_number(candidate: &str) -> bool {
    let digits: Vec<u8> = candidate
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|byte| byte - b'0')
        .collect();
    (13..=19).contains(&digits.len())
        && has_issuer_prefix(&digits)
        && !all_same(&digits)
        && passes_luhn(&digits)
}

/// Visa, Mastercard, American Express, Discover, JCB and Diners Club.
fn has_issuer_prefix(digits: &[u8]) -> bool {
    matches!(
        digits,
        [4, ..]
            | [5, 1..=5, ..]
            | [2, 2..=7, ..]
            | [3, 0 | 4 | 5 | 6 | 7 | 8, ..]
            | [6, 0, 1, 1, ..]
            | [6, 5, ..]
    )
}

fn all_same(digits: &[u8]) -> bool {
    digits.windows(2).all(|pair| pair[0] == pair[1])
}

fn passes_luhn(digits: &[u8]) -> bool {
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(position, &digit)| {
            if position % 2 == 1 {
                DOUBLED[usize::from(digit)]
            } else {
                u32::from(digit)
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

#[cfg(test)]
mod tests {
    use super::{all_same, contains_card_number, has_issuer_prefix, is_card_number, passes_luhn};

    fn digits(s: &str) -> Vec<u8> {
        s.bytes().map(|b| b - b'0').collect()
    }

    #[test]
    fn luhn_accepts_valid_and_rejects_off_by_one() {
        assert!(passes_luhn(&digits("4111111111111111")));
        assert!(passes_luhn(&digits("79927398713")));
        assert!(!passes_luhn(&digits("4111111111111112")));
        assert!(!passes_luhn(&digits("79927398710")));
    }

    #[test]
    fn luhn_doubles_every_second_digit_from_the_right() {
        // "18": 8 + double(1) = 10, valid. "81": 1 + double(8) = 1 + 7 = 8, invalid.
        assert!(passes_luhn(&digits("18")));
        assert!(!passes_luhn(&digits("81")));
        assert!(passes_luhn(&digits("59")));
        assert!(passes_luhn(&digits("091")));
    }

    #[test]
    fn issuer_prefixes() {
        for number in [
            "4", "51", "55", "22", "27", "30", "34", "35", "36", "37", "38", "6011", "65",
        ] {
            assert!(has_issuer_prefix(&digits(number)), "{number}");
        }
        for number in [
            "1", "50", "56", "21", "28", "31", "32", "33", "39", "6010", "64", "7", "9", "",
        ] {
            assert!(!has_issuer_prefix(&digits(number)), "{number}");
        }
    }

    #[test]
    fn repeated_digits_are_not_cards() {
        assert!(all_same(&digits("4444444444444")));
        assert!(!all_same(&digits("4444444444441")));
        // Seventeen 4s pass Luhn and carry a Visa prefix; only the repetition rule rejects them.
        assert!(passes_luhn(&digits("44444444444444444")));
        assert!(!is_card_number("44444444444444444"));
    }

    #[test]
    fn length_bounds() {
        assert!(is_card_number("4222222222222"));
        assert!(!is_card_number("422222222222"));
        assert!(is_card_number("4000000000000000006"));
        assert!(!is_card_number("40000000000000000006"));
    }

    #[test]
    fn grouped_numbers_are_found_inside_text() {
        assert!(contains_card_number("card: 4111 1111 1111 1111, exp 12/30"));
        assert!(contains_card_number("5555-5555-5555-4444"));
        assert!(!contains_card_number("order 4111111111111112"));
        assert!(!contains_card_number("41111111111111111111111"));
    }
}
