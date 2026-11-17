use super::{History, HistoryError, Settings};
use crate::item::{Capture, ItemId, Kind, Reason};
use crate::{Content, Sensitivity};
use copyveil_detect::{Detector, DetectorSet};
use std::time::{Duration, Instant};

const AWS: &str = "AKIAWBHK8C2D086BCGWN";
const SECOND: Duration = Duration::from_secs(1);
const NANO: Duration = Duration::from_nanos(1);

fn history() -> History {
    History::new(Settings::default())
}

fn with_limit(max_plain_items: usize) -> History {
    History::new(Settings {
        max_plain_items,
        ..Settings::default()
    })
}

fn ids(history: &History) -> Vec<ItemId> {
    history.items().iter().map(crate::Item::id).collect()
}

fn rich(content: &str, fallback: Option<&str>) -> Capture {
    Capture {
        content: Content::from(content),
        kind: Kind::RichText,
        plain_fallback: fallback.map(Content::from),
        password_manager_hint: false,
    }
}

fn image(bytes: &str) -> Capture {
    Capture {
        content: Content::from(bytes),
        kind: Kind::Image {
            mime: "image/png".into(),
        },
        plain_fallback: None,
        password_manager_hint: false,
    }
}

mod settings {
    use super::*;

    #[test]
    fn defaults() {
        let settings = Settings::default();
        assert_eq!(settings.secret_expiry, Duration::from_secs(30));
        assert_eq!(settings.suspected_expiry, Duration::from_mins(15));
        assert_eq!(settings.max_plain_items, 500);
        assert_eq!(settings.detectors, DetectorSet::all());
        assert_eq!(history().settings(), &settings);
    }

    #[test]
    fn disabled_detectors_leave_text_plain() {
        let mut history = History::new(Settings {
            detectors: DetectorSet::none(),
            ..Settings::default()
        });
        let id = history.capture(Capture::text(AWS), Instant::now());
        assert_eq!(history.get(id).unwrap().sensitivity(), Sensitivity::Plain);
    }

    #[test]
    fn lowering_the_limit_evicts_immediately() {
        let mut history = history();
        let now = Instant::now();
        history.capture(Capture::text("one"), now);
        let two = history.capture(Capture::text("two"), now);
        history.set_settings(Settings {
            max_plain_items: 1,
            ..Settings::default()
        });
        assert_eq!(ids(&history), [two]);
        assert_eq!(history.settings().max_plain_items, 1);
    }
}

mod capture {
    use super::*;

    #[test]
    fn newest_first_with_unique_ids() {
        let mut history = history();
        let now = Instant::now();
        let first = history.capture(Capture::text("first"), now);
        let second = history.capture(Capture::text("second"), now);
        assert_ne!(first, second);
        assert_eq!(ids(&history), [second, first]);
        assert_eq!(history.get(first).unwrap().content().expose(), b"first");
    }

    #[test]
    fn records_what_was_captured() {
        let mut history = history();
        let now = Instant::now();
        let id = history.capture(rich("<b>hi</b>", Some("hi")), now);
        let item = history.get(id).unwrap();
        assert_eq!(item.kind(), &Kind::RichText);
        assert_eq!(item.plain_fallback().unwrap().expose(), b"hi");
        assert_eq!(item.captured_at(), now);
        assert_eq!(item.reason(), Reason::Unflagged);
    }

    #[test]
    fn expiry_depends_on_sensitivity() {
        let mut history = history();
        let now = Instant::now();
        let plain = history.capture(Capture::text("ls -la"), now);
        let suspected = history.capture(Capture::text(AWS), now);
        let secret = history.capture(Capture::secret_text("hunter2"), now);

        assert_eq!(history.get(plain).unwrap().expires_at(), None);
        assert_eq!(
            history.get(suspected).unwrap().expires_at(),
            Some(now + Duration::from_mins(15))
        );
        assert_eq!(
            history.get(secret).unwrap().expires_at(),
            Some(now + Duration::from_secs(30))
        );
        assert_eq!(
            history.get(suspected).unwrap().reason(),
            Reason::Detected(Detector::AwsAccessKey)
        );
        assert_eq!(
            history.get(secret).unwrap().reason(),
            Reason::PasswordManager
        );
    }
}

mod deduplication {
    use super::*;

    #[test]
    fn copying_again_moves_the_item_to_the_top() {
        let mut history = history();
        let now = Instant::now();
        let first = history.capture(Capture::text("again"), now);
        history.capture(Capture::text("other"), now);
        let later = now + SECOND;
        let again = history.capture(Capture::text("again"), later);

        assert_eq!(again, first);
        assert_eq!(history.items().len(), 2);
        assert_eq!(history.items()[0].id(), first);
        assert_eq!(history.get(first).unwrap().captured_at(), later);
    }

    #[test]
    fn copying_again_restarts_the_expiry() {
        let mut history = history();
        let now = Instant::now();
        let id = history.capture(Capture::secret_text("hunter2"), now);
        history.capture(Capture::secret_text("hunter2"), now + 10 * SECOND);
        assert_eq!(
            history.get(id).unwrap().expires_at(),
            Some(now + 40 * SECOND)
        );
    }

    #[test]
    fn same_bytes_of_a_different_kind_are_separate_items() {
        let mut history = history();
        let now = Instant::now();
        let text = history.capture(Capture::text("hi"), now);
        let html = history.capture(rich("hi", None), now);
        assert_ne!(text, html);
        assert_eq!(history.items().len(), 2);
    }

    #[test]
    fn an_override_survives_copying_again() {
        let mut history = history();
        let now = Instant::now();
        let id = history.capture(Capture::text(AWS), now);
        history
            .override_sensitivity(id, Sensitivity::Plain, now)
            .unwrap();
        history.capture(Capture::text(AWS), now);
        let item = history.get(id).unwrap();
        assert_eq!(item.sensitivity(), Sensitivity::Plain);
        assert_eq!(item.reason(), Reason::Override);
        assert_eq!(item.expires_at(), None);
    }

    #[test]
    fn a_password_manager_hint_upgrades_and_unpins() {
        let mut history = history();
        let now = Instant::now();
        let id = history.capture(Capture::text("hunter2"), now);
        history.pin(id).unwrap();
        history.capture(Capture::secret_text("hunter2"), now);
        let item = history.get(id).unwrap();
        assert_eq!(item.sensitivity(), Sensitivity::Secret);
        assert_eq!(item.reason(), Reason::PasswordManager);
        assert!(!history.is_pinned(id));
    }

    #[test]
    fn a_pinned_plain_item_stays_pinned_when_copied_again() {
        let mut history = history();
        let now = Instant::now();
        let id = history.capture(Capture::text("ssh prod"), now);
        history.pin(id).unwrap();
        history.capture(Capture::text("ssh prod"), now);
        assert!(history.is_pinned(id));
    }
}

mod expiry {
    use super::*;

    #[test]
    fn items_expire_exactly_at_their_expiry() {
        let mut history = history();
        let now = Instant::now();
        let secret = history.capture(Capture::secret_text("hunter2"), now);
        let plain = history.capture(Capture::text("ls"), now);

        assert!(
            history
                .expire(now + (30 * SECOND).checked_sub(NANO).unwrap())
                .is_empty()
        );
        assert!(history.get(secret).is_some());

        assert_eq!(history.expire(now + 30 * SECOND), [secret]);
        assert!(history.get(secret).is_none());
        assert!(history.get(plain).is_some());
        assert!(history.expire(now + Duration::from_hours(24)).is_empty());
    }

    #[test]
    fn next_expiry_is_the_earliest() {
        let mut history = history();
        let now = Instant::now();
        assert_eq!(history.next_expiry(), None);
        history.capture(Capture::text("ls"), now);
        assert_eq!(history.next_expiry(), None);
        history.capture(Capture::text(AWS), now);
        history.capture(Capture::secret_text("hunter2"), now + SECOND);
        assert_eq!(history.next_expiry(), Some(now + 31 * SECOND));
    }
}

mod pins {
    use super::*;

    fn three_pins() -> (History, [ItemId; 3]) {
        let mut history = history();
        let now = Instant::now();
        let a = history.capture(Capture::text("a"), now);
        let b = history.capture(Capture::text("b"), now);
        let c = history.capture(Capture::text("c"), now);
        for id in [a, b, c] {
            history.pin(id).unwrap();
        }
        (history, [a, b, c])
    }

    fn slots(history: &History) -> Vec<ItemId> {
        history.pins().map(crate::Item::id).collect()
    }

    #[test]
    fn only_plain_items_can_be_pinned() {
        let mut history = history();
        let now = Instant::now();
        let plain = history.capture(Capture::text("ls"), now);
        let suspected = history.capture(Capture::text(AWS), now);
        let secret = history.capture(Capture::secret_text("hunter2"), now);

        assert_eq!(history.pin(plain), Ok(()));
        assert_eq!(
            history.pin(suspected),
            Err(HistoryError::NotPlain(suspected))
        );
        assert_eq!(history.pin(secret), Err(HistoryError::NotPlain(secret)));
        assert_eq!(
            history.pin(ItemId(999)),
            Err(HistoryError::UnknownItem(ItemId(999)))
        );
        assert_eq!(slots(&history), [plain]);
    }

    #[test]
    fn pinning_twice_keeps_one_slot() {
        let mut history = history();
        let id = history.capture(Capture::text("ls"), Instant::now());
        history.pin(id).unwrap();
        history.pin(id).unwrap();
        assert_eq!(slots(&history), [id]);
    }

    #[test]
    fn pin_slots_count_from_one_in_pin_order() {
        let (history, [a, b, c]) = three_pins();
        assert!(history.pin_slot(0).is_none());
        assert_eq!(history.pin_slot(1).unwrap().id(), a);
        assert_eq!(history.pin_slot(2).unwrap().id(), b);
        assert_eq!(history.pin_slot(3).unwrap().id(), c);
        assert!(history.pin_slot(4).is_none());
    }

    #[test]
    fn unpinning_moves_later_pins_up() {
        let (mut history, [a, b, c]) = three_pins();
        history.unpin(a).unwrap();
        assert!(!history.is_pinned(a));
        assert_eq!(slots(&history), [b, c]);
        assert!(history.get(a).is_some(), "unpinning keeps the Item");
        assert_eq!(
            history.unpin(ItemId(999)),
            Err(HistoryError::UnknownItem(ItemId(999)))
        );
    }

    #[test]
    fn move_pin_reorders_slots() {
        let (mut history, [a, b, c]) = three_pins();
        history.move_pin(c, 1).unwrap();
        assert_eq!(slots(&history), [c, a, b]);
        history.move_pin(c, 3).unwrap();
        assert_eq!(slots(&history), [a, b, c]);
    }

    #[test]
    fn move_pin_rejects_bad_slots_and_unpinned_items() {
        let (mut history, [a, ..]) = three_pins();
        assert_eq!(history.move_pin(a, 0), Err(HistoryError::NoSuchPinSlot(0)));
        assert_eq!(history.move_pin(a, 4), Err(HistoryError::NoSuchPinSlot(4)));
        history.unpin(a).unwrap();
        assert_eq!(history.move_pin(a, 1), Err(HistoryError::NotPinned(a)));
    }

    #[test]
    fn removing_an_item_frees_its_slot() {
        let (mut history, [a, b, c]) = three_pins();
        history.remove(b).unwrap();
        assert!(history.get(b).is_none());
        assert_eq!(slots(&history), [a, c]);
        assert_eq!(history.remove(b), Err(HistoryError::UnknownItem(b)));
    }
}

mod overrides {
    use super::*;

    #[test]
    fn promoting_starts_an_expiry_and_unpins() {
        let mut history = history();
        let now = Instant::now();
        let id = history.capture(Capture::text("ls"), now);
        history.pin(id).unwrap();
        let later = now + SECOND;

        history
            .override_sensitivity(id, Sensitivity::Secret, later)
            .unwrap();
        let item = history.get(id).unwrap();
        assert_eq!(item.sensitivity(), Sensitivity::Secret);
        assert_eq!(item.reason(), Reason::Override);
        assert_eq!(item.expires_at(), Some(later + 30 * SECOND));
        assert!(!history.is_pinned(id));

        history
            .override_sensitivity(id, Sensitivity::Suspected, later)
            .unwrap();
        assert_eq!(
            history.get(id).unwrap().expires_at(),
            Some(later + 900 * SECOND)
        );
    }

    #[test]
    fn demoting_to_plain_clears_the_expiry_and_allows_pinning() {
        let mut history = history();
        let now = Instant::now();
        let id = history.capture(Capture::text(AWS), now);
        history
            .override_sensitivity(id, Sensitivity::Plain, now)
            .unwrap();
        assert_eq!(history.get(id).unwrap().expires_at(), None);
        assert_eq!(history.pin(id), Ok(()));
        assert!(history.expire(now + Duration::from_hours(24)).is_empty());
    }

    #[test]
    fn unknown_items_are_refused() {
        assert_eq!(
            history().override_sensitivity(ItemId(1), Sensitivity::Plain, Instant::now()),
            Err(HistoryError::UnknownItem(ItemId(1)))
        );
    }
}

mod eviction {
    use super::*;

    #[test]
    fn oldest_plain_items_go_first() {
        let mut history = with_limit(2);
        let now = Instant::now();
        history.capture(Capture::text("1"), now);
        let two = history.capture(Capture::text("2"), now);
        let three = history.capture(Capture::text("3"), now);
        assert_eq!(ids(&history), [three, two]);
    }

    #[test]
    fn pins_and_masked_items_do_not_count() {
        let mut history = with_limit(1);
        let now = Instant::now();
        let pinned = history.capture(Capture::text("pinned"), now);
        history.pin(pinned).unwrap();
        let secret = history.capture(Capture::secret_text("hunter2"), now);
        let suspected = history.capture(Capture::text(AWS), now);
        let plain = history.capture(Capture::text("plain"), now);
        assert_eq!(ids(&history), [plain, suspected, secret, pinned]);
    }

    #[test]
    fn unpinning_applies_the_limit() {
        let mut history = with_limit(1);
        let now = Instant::now();
        let old = history.capture(Capture::text("old"), now);
        history.pin(old).unwrap();
        let new = history.capture(Capture::text("new"), now);
        history.unpin(old).unwrap();
        assert_eq!(ids(&history), [new]);
    }

    #[test]
    fn demoting_applies_the_limit() {
        let mut history = with_limit(1);
        let now = Instant::now();
        let suspected = history.capture(Capture::text(AWS), now);
        let plain = history.capture(Capture::text("plain"), now);
        history
            .override_sensitivity(suspected, Sensitivity::Plain, now)
            .unwrap();
        assert_eq!(ids(&history), [plain]);
    }
}

mod search {
    use super::*;

    #[test]
    fn plain_items_match_their_text_case_insensitively() {
        let mut history = history();
        let now = Instant::now();
        let compose = history.capture(Capture::text("docker compose up -d"), now);
        history.capture(Capture::text("git status"), now);
        assert_eq!(history.search("COMPOSE"), [compose]);
        assert!(history.search("kubectl").is_empty());
    }

    #[test]
    fn masked_items_match_only_their_description() {
        let mut history = history();
        let now = Instant::now();
        let suspected = history.capture(Capture::text(AWS), now);
        let secret = history.capture(Capture::secret_text("hunter2"), now);

        assert!(history.search("AKIA").is_empty());
        assert!(history.search("hunter").is_empty());
        assert_eq!(history.search("aws access key"), [suspected]);
        assert_eq!(history.search("password manager"), [secret]);
    }

    #[test]
    fn an_empty_query_matches_everything() {
        let mut history = history();
        let now = Instant::now();
        let a = history.capture(Capture::text("a"), now);
        let b = history.capture(Capture::secret_text("b"), now);
        assert_eq!(history.search(""), [b, a]);
    }

    #[test]
    fn rich_text_and_images() {
        let mut history = history();
        let now = Instant::now();
        let with_fallback =
            history.capture(rich("<i>meeting notes</i>", Some("meeting notes")), now);
        let without = history.capture(rich("<p>standup</p>", None), now);
        let png = history.capture(image("\u{89}PNG"), now);

        assert_eq!(history.search("meeting"), [with_fallback]);
        assert!(
            history.search("<i>").is_empty(),
            "uses the fallback when there is one"
        );
        assert_eq!(history.search("standup"), [without]);
        assert_eq!(history.search("image/png"), [png]);
    }
}

mod errors {
    use super::*;

    #[test]
    fn messages_name_ids_not_content() {
        let id = ItemId(4);
        assert_eq!(
            HistoryError::UnknownItem(id).to_string(),
            "no Item with id 4"
        );
        assert_eq!(
            HistoryError::NotPlain(id).to_string(),
            "Item 4 is not Plain, so it can't be pinned"
        );
        assert_eq!(
            HistoryError::NotPinned(id).to_string(),
            "Item 4 is not pinned"
        );
        assert_eq!(
            HistoryError::NoSuchPinSlot(10).to_string(),
            "there is no Pin slot 10"
        );
    }
}
