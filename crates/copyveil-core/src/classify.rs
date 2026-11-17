//! Deciding a Capture's Sensitivity.

use crate::Sensitivity;
use crate::item::{Capture, Kind, Reason};
use copyveil_detect::{DetectorSet, detect};

/// A password manager hint makes a Secret. Otherwise the detectors decide
/// between Suspected and Plain, reading text, or rich text's plain fallback.
/// Images are always Plain: detectors read text, not pixels.
pub(crate) fn classify(capture: &Capture, detectors: DetectorSet) -> (Sensitivity, Reason) {
    if capture.password_manager_hint {
        return (Sensitivity::Secret, Reason::PasswordManager);
    }
    let text = match capture.kind {
        Kind::Text => Some(&capture.content),
        Kind::RichText => capture.plain_fallback.as_ref().or(Some(&capture.content)),
        Kind::Image { .. } => None,
    };
    let found = text.and_then(|content| {
        let text = String::from_utf8_lossy(content.expose());
        detect(&text, detectors)
    });
    match found {
        Some(detector) => (Sensitivity::Suspected, Reason::Detected(detector)),
        None => (Sensitivity::Plain, Reason::Unflagged),
    }
}

#[cfg(test)]
mod tests {
    use super::classify;
    use crate::item::{Capture, Kind, Reason};
    use crate::{Content, Sensitivity};
    use copyveil_detect::{Detector, DetectorSet};

    const AWS: &str = "AKIAWBHK8C2D086BCGWN";

    #[test]
    fn password_manager_hint_makes_a_secret() {
        assert_eq!(
            classify(&Capture::secret_text("plain words"), DetectorSet::all()),
            (Sensitivity::Secret, Reason::PasswordManager)
        );
    }

    #[test]
    fn detectors_make_suspected() {
        assert_eq!(
            classify(&Capture::text(AWS), DetectorSet::all()),
            (
                Sensitivity::Suspected,
                Reason::Detected(Detector::AwsAccessKey)
            )
        );
        assert_eq!(
            classify(&Capture::text(AWS), DetectorSet::none()),
            (Sensitivity::Plain, Reason::Unflagged)
        );
    }

    #[test]
    fn ordinary_text_is_plain() {
        assert_eq!(
            classify(&Capture::text("docker compose up -d"), DetectorSet::all()),
            (Sensitivity::Plain, Reason::Unflagged)
        );
    }

    #[test]
    fn rich_text_is_read_through_its_fallback() {
        let capture = Capture {
            content: Content::from("<b>key</b>"),
            kind: Kind::RichText,
            plain_fallback: Some(Content::from(AWS)),
            password_manager_hint: false,
        };
        assert_eq!(
            classify(&capture, DetectorSet::all()).0,
            Sensitivity::Suspected
        );

        let without_fallback = Capture {
            content: Content::from(AWS),
            kind: Kind::RichText,
            plain_fallback: None,
            password_manager_hint: false,
        };
        assert_eq!(
            classify(&without_fallback, DetectorSet::all()).0,
            Sensitivity::Suspected
        );
    }

    #[test]
    fn images_are_never_scanned() {
        let capture = Capture {
            content: Content::from(AWS),
            kind: Kind::Image {
                mime: "image/png".into(),
            },
            plain_fallback: None,
            password_manager_hint: false,
        };
        assert_eq!(
            classify(&capture, DetectorSet::all()),
            (Sensitivity::Plain, Reason::Unflagged)
        );
    }
}
