//! Detector corpus (see docs/testing.md).
//!
//! Every line of `<detector>.positive.txt` must be reported as that detector.
//! No line of any `.negative.txt` may be flagged by any detector.
//! Blank lines and lines starting with `#` are ignored.

use copyveil_detect::{Detector, DetectorSet, detect};

const CORPUS: [(Detector, &str, &str); 7] = [
    (
        Detector::GitHubToken,
        include_str!("corpus/github-token.positive.txt"),
        include_str!("corpus/github-token.negative.txt"),
    ),
    (
        Detector::AwsAccessKey,
        include_str!("corpus/aws-access-key.positive.txt"),
        include_str!("corpus/aws-access-key.negative.txt"),
    ),
    (
        Detector::AiProviderKey,
        include_str!("corpus/ai-provider-key.positive.txt"),
        include_str!("corpus/ai-provider-key.negative.txt"),
    ),
    (
        Detector::Jwt,
        include_str!("corpus/jwt.positive.txt"),
        include_str!("corpus/jwt.negative.txt"),
    ),
    (
        Detector::PrivateKey,
        include_str!("corpus/private-key.positive.txt"),
        include_str!("corpus/private-key.negative.txt"),
    ),
    (
        Detector::PaymentCard,
        include_str!("corpus/payment-card.positive.txt"),
        include_str!("corpus/payment-card.negative.txt"),
    ),
    (
        Detector::HighEntropy,
        include_str!("corpus/high-entropy.positive.txt"),
        include_str!("corpus/high-entropy.negative.txt"),
    ),
];

fn samples(file: &str) -> impl Iterator<Item = (usize, &str)> {
    file.lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line))
        .filter(|(_, line)| !line.trim().is_empty() && !line.starts_with('#'))
}

#[test]
fn every_detector_has_a_corpus() {
    let covered: Vec<_> = CORPUS.iter().map(|(detector, _, _)| *detector).collect();
    assert_eq!(covered, Detector::ALL);
}

#[test]
fn positives_are_reported_as_their_detector() {
    let mut failures = Vec::new();
    for (detector, positives, _) in CORPUS {
        for (line, sample) in samples(positives) {
            let found = detect(sample, DetectorSet::all());
            if found != Some(detector) {
                failures.push(format!("{detector:?} positive line {line}: got {found:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn negatives_are_not_flagged() {
    let mut failures = Vec::new();
    for (detector, _, negatives) in CORPUS {
        for (line, sample) in samples(negatives) {
            if let Some(found) = detect(sample, DetectorSet::all()) {
                failures.push(format!(
                    "{detector:?} negative line {line}: flagged as {found:?}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
