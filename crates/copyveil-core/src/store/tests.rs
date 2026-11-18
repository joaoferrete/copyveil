use super::{MAX_PERSISTED_BYTES, Opened, Store, StoreError, erase_files, instant_at, wall_ms};
use crate::crypto::{CryptoError, Key};
use crate::{Capture, Content, History, Kind, Reason, Sensitivity, Settings};
use copyveil_detect::DetectorSet;
use std::error::Error;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

const SECOND: Duration = Duration::from_secs(1);
const AWS: &str = "AKIAWBHK8C2D086BCGWN";

struct Fixture {
    dir: TempDir,
    key: [u8; 32],
    now: Instant,
    wall: SystemTime,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
            key: *Key::generate().unwrap().expose(),
            now: Instant::now(),
            wall: UNIX_EPOCH + Duration::from_secs(1_763_400_123),
        }
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("history.db")
    }

    fn key(&self) -> Key {
        Key::from_bytes(&self.key).unwrap()
    }

    fn open(&self) -> (Store, Opened) {
        Store::open(&self.path(), self.key()).unwrap()
    }

    /// Opens the store again `later` after `now` and loads it.
    fn reload(&self, settings: Settings, later: Duration) -> (Store, History, usize) {
        let (mut store, opened) = self.open();
        assert_eq!(opened, Opened::Existing);
        let loaded = store
            .load(settings, self.now + later, self.wall + later)
            .unwrap();
        (store, loaded.history, loaded.unreadable)
    }

    fn sync(&self, store: &mut Store, history: &History) {
        store.sync(history, self.now, self.wall).unwrap();
    }
}

fn rows(store: &Store) -> Vec<(i64, Vec<u8>, Vec<u8>)> {
    store
        .conn
        .prepare("SELECT id, meta, content FROM items ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn texts(history: &History) -> Vec<String> {
    history
        .items()
        .iter()
        .map(|item| String::from_utf8(item.content().expose().to_vec()).unwrap())
        .collect()
}

fn image(bytes: Vec<u8>) -> Capture {
    Capture {
        content: Content::new(bytes),
        kind: Kind::Image {
            mime: "image/png".into(),
        },
        plain_fallback: None,
        password_manager_hint: false,
    }
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

#[test]
fn a_new_store_is_private_and_erases_freed_pages() {
    let fx = Fixture::new();
    let (store, opened) = fx.open();
    assert_eq!(opened, Opened::Created);
    let mode = std::fs::metadata(fx.path()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    let secure_delete: i64 = store
        .conn
        .pragma_query_value(None, "secure_delete", |row| row.get(0))
        .unwrap();
    assert_eq!(secure_delete, 1);
}

#[test]
fn plain_items_survive_a_restart_in_order() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("first"), fx.now);
    history.capture(Capture::text("second"), fx.now + SECOND);
    history.capture(Capture::text("third"), fx.now + 2 * SECOND);
    store
        .sync(&history, fx.now + 2 * SECOND, fx.wall + 2 * SECOND)
        .unwrap();
    drop(store);

    let (_, restored, unreadable) = fx.reload(Settings::default(), 10 * SECOND);
    assert_eq!(unreadable, 0);
    assert_eq!(texts(&restored), ["third", "second", "first"]);
    let captured: Vec<Instant> = restored
        .items()
        .iter()
        .map(crate::Item::captured_at)
        .collect();
    assert_eq!(captured, [fx.now + 2 * SECOND, fx.now + SECOND, fx.now]);
    for item in restored.items() {
        assert_eq!(item.sensitivity(), Sensitivity::Plain);
        assert_eq!(item.reason(), Reason::Unflagged);
        assert_eq!(item.expires_at(), None);
    }
}

#[test]
fn every_kind_round_trips() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(image(vec![0x89, b'P', b'N', b'G']), fx.now);
    history.capture(
        Capture {
            content: Content::from("<b>bold</b>"),
            kind: Kind::RichText,
            plain_fallback: Some(Content::from("bold")),
            password_manager_hint: false,
        },
        fx.now,
    );
    fx.sync(&mut store, &history);
    drop(store);

    let (_, restored, _) = fx.reload(Settings::default(), SECOND);
    let [rich, picture] = restored.items() else {
        panic!("expected two Items");
    };
    assert_eq!(rich.kind(), &Kind::RichText);
    assert_eq!(rich.content().expose(), b"<b>bold</b>");
    assert_eq!(rich.plain_fallback().unwrap().expose(), b"bold");
    assert_eq!(
        picture.kind(),
        &Kind::Image {
            mime: "image/png".into()
        }
    );
    assert_eq!(picture.content().expose(), [0x89, b'P', b'N', b'G']);
    assert!(picture.plain_fallback().is_none());
}

#[test]
fn secret_and_suspected_items_are_never_written() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::secret_text("hunter2"), fx.now);
    history.capture(Capture::text(AWS), fx.now);
    let plain = history.capture(Capture::text("ls -la"), fx.now);
    fx.sync(&mut store, &history);

    let stored = rows(&store);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].0.cast_unsigned(), plain.0);
}

#[test]
fn ids_continue_after_the_largest_restored_one() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("a"), fx.now);
    let b = history.capture(Capture::text("b"), fx.now);
    fx.sync(&mut store, &history);
    drop(store);

    let (_, mut restored, _) = fx.reload(Settings::default(), SECOND);
    let c = restored.capture(Capture::text("c"), fx.now);
    assert!(c > b);
    assert!(restored.get(b).is_some());
}

#[test]
fn new_items_after_a_restart_stay_on_top() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("a"), fx.now);
    history.capture(Capture::text("b"), fx.now);
    fx.sync(&mut store, &history);
    drop(store);

    let (mut store, mut restored, _) = fx.reload(Settings::default(), SECOND);
    restored.capture(Capture::text("c"), fx.now + SECOND);
    store
        .sync(&restored, fx.now + SECOND, fx.wall + SECOND)
        .unwrap();
    drop(store);

    let (_, again, _) = fx.reload(Settings::default(), 2 * SECOND);
    assert_eq!(texts(&again), ["c", "b", "a"]);
}

#[test]
fn copying_again_moves_an_item_up_on_disk_too() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("a"), fx.now);
    history.capture(Capture::text("b"), fx.now);
    fx.sync(&mut store, &history);
    history.capture(Capture::text("a"), fx.now + SECOND);
    store
        .sync(&history, fx.now + SECOND, fx.wall + SECOND)
        .unwrap();
    drop(store);

    let (_, restored, _) = fx.reload(Settings::default(), 2 * SECOND);
    assert_eq!(texts(&restored), ["a", "b"]);
    assert_eq!(restored.items()[0].captured_at(), fx.now + SECOND);
}

#[test]
fn unchanged_items_are_not_rewritten() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("a"), fx.now);
    fx.sync(&mut store, &history);
    let before = rows(&store);
    store
        .sync(&history, fx.now + SECOND, fx.wall + SECOND)
        .unwrap();
    assert_eq!(rows(&store), before);
}

#[test]
fn pinning_rewrites_only_the_meta_record() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    let a = history.capture(Capture::text("a"), fx.now);
    let b = history.capture(Capture::text("b"), fx.now);
    fx.sync(&mut store, &history);
    let before = rows(&store);

    history.pin(b).unwrap();
    history.pin(a).unwrap();
    fx.sync(&mut store, &history);
    let after = rows(&store);
    for (old, new) in before.iter().zip(&after) {
        assert_ne!(old.1, new.1, "meta of row {}", old.0);
        assert_eq!(old.2, new.2, "content of row {}", old.0);
    }
    drop(store);

    let (_, restored, _) = fx.reload(Settings::default(), SECOND);
    let pins: Vec<_> = restored.pins().map(crate::Item::id).collect();
    assert_eq!(pins, [b, a]);
}

#[test]
fn unpinning_and_reordering_pins_persist() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    let a = history.capture(Capture::text("a"), fx.now);
    let b = history.capture(Capture::text("b"), fx.now);
    let c = history.capture(Capture::text("c"), fx.now);
    for id in [a, b, c] {
        history.pin(id).unwrap();
    }
    fx.sync(&mut store, &history);
    history.unpin(a).unwrap();
    history.move_pin(c, 1).unwrap();
    fx.sync(&mut store, &history);
    drop(store);

    let (_, restored, _) = fx.reload(Settings::default(), SECOND);
    let pins: Vec<_> = restored.pins().map(crate::Item::id).collect();
    assert_eq!(pins, [c, b]);
    assert!(!restored.is_pinned(a));
}

#[test]
fn removed_evicted_and_promoted_items_leave_the_disk() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    let removed = history.capture(Capture::text("removed"), fx.now);
    let promoted = history.capture(Capture::text("promoted"), fx.now);
    let evicted = history.capture(Capture::text("evicted"), fx.now);
    let kept = history.capture(Capture::text("kept"), fx.now);
    fx.sync(&mut store, &history);
    assert_eq!(rows(&store).len(), 4);

    history.remove(removed).unwrap();
    history
        .override_sensitivity(promoted, Sensitivity::Secret, fx.now)
        .unwrap();
    history.set_settings(Settings {
        max_plain_items: 1,
        ..Settings::default()
    });
    assert!(history.get(evicted).is_none());
    fx.sync(&mut store, &history);

    let stored: Vec<u64> = rows(&store)
        .iter()
        .map(|row| row.0.cast_unsigned())
        .collect();
    assert_eq!(stored, [kept.0]);
}

#[test]
fn demoting_an_item_writes_it_and_the_override_survives() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    let id = history.capture(Capture::text(AWS), fx.now);
    fx.sync(&mut store, &history);
    assert!(rows(&store).is_empty());

    history
        .override_sensitivity(id, Sensitivity::Plain, fx.now)
        .unwrap();
    fx.sync(&mut store, &history);
    assert_eq!(rows(&store).len(), 1);
    drop(store);

    let (_, restored, _) = fx.reload(Settings::default(), SECOND);
    let item = restored.get(id).unwrap();
    assert_eq!(item.sensitivity(), Sensitivity::Plain);
    assert_eq!(item.reason(), Reason::Override);
}

#[test]
fn newer_detectors_hide_items_stored_as_plain() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let blind = Settings {
        detectors: DetectorSet::none(),
        ..Settings::default()
    };
    let mut history = History::new(blind);
    let key = history.capture(Capture::text(AWS), fx.now);
    history.pin(key).unwrap();
    history.capture(Capture::text("ls"), fx.now);
    fx.sync(&mut store, &history);
    drop(store);

    let (mut store, restored, _) = fx.reload(Settings::default(), 10 * SECOND);
    let item = restored.get(key).unwrap();
    assert_eq!(item.sensitivity(), Sensitivity::Suspected);
    assert_eq!(
        item.expires_at(),
        Some(fx.now + 10 * SECOND + Settings::default().suspected_expiry)
    );
    assert!(!restored.is_pinned(key));
    assert_eq!(restored.pins().count(), 0);

    fx.sync(&mut store, &restored);
    assert_eq!(rows(&store).len(), 1);
}

#[test]
fn a_lower_limit_applies_on_load() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    for text in ["a", "b", "c"] {
        history.capture(Capture::text(text), fx.now);
    }
    fx.sync(&mut store, &history);
    drop(store);

    let small = Settings {
        max_plain_items: 2,
        ..Settings::default()
    };
    let (_, restored, _) = fx.reload(small, SECOND);
    assert_eq!(texts(&restored), ["c", "b"]);
}

#[test]
fn items_up_to_the_size_limit_are_written() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    let fits = history.capture(image(vec![1; MAX_PERSISTED_BYTES]), fx.now);
    history.capture(image(vec![2; MAX_PERSISTED_BYTES + 1]), fx.now);
    history.capture(
        Capture {
            content: Content::new(vec![b'a'; MAX_PERSISTED_BYTES - 1]),
            kind: Kind::RichText,
            plain_fallback: Some(Content::from("ab")),
            password_manager_hint: false,
        },
        fx.now,
    );
    fx.sync(&mut store, &history);

    let stored: Vec<u64> = rows(&store)
        .iter()
        .map(|row| row.0.cast_unsigned())
        .collect();
    assert_eq!(stored, [fits.0]);
}

#[test]
fn a_different_key_wipes_the_store() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("a"), fx.now);
    fx.sync(&mut store, &history);
    drop(store);

    let (mut store, opened) = Store::open(&fx.path(), Key::generate().unwrap()).unwrap();
    assert_eq!(opened, Opened::Wiped);
    let loaded = store.load(Settings::default(), fx.now, fx.wall).unwrap();
    assert!(loaded.history.items().is_empty());
    assert_eq!(loaded.unreadable, 0);
    let mode = std::fs::metadata(fx.path()).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    drop(store);

    assert_eq!(fx.open().1, Opened::Wiped);
}

#[test]
fn a_file_that_is_not_a_database_is_wiped() {
    let fx = Fixture::new();
    std::fs::write(fx.path(), vec![0xa5; 4096]).unwrap();
    std::fs::write(fx.dir.path().join("history.db-journal"), b"stale").unwrap();
    let (mut store, opened) = fx.open();
    assert_eq!(opened, Opened::Wiped);
    assert!(
        store
            .load(Settings::default(), fx.now, fx.wall)
            .unwrap()
            .history
            .items()
            .is_empty()
    );
    assert_eq!(files_in(fx.dir.path()), ["history.db"]);
}

#[test]
fn rows_moved_or_tampered_with_are_erased() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    let a = history.capture(Capture::text("a"), fx.now);
    let b = history.capture(Capture::text("b"), fx.now);
    let c = history.capture(Capture::text("c"), fx.now);
    history.capture(Capture::text("d"), fx.now);
    fx.sync(&mut store, &history);

    let stored = rows(&store);
    let meta_of = |id: crate::ItemId| {
        stored
            .iter()
            .find(|row| row.0.cast_unsigned() == id.0)
            .unwrap()
            .clone()
    };
    // a gets b's meta (moved between rows); c gets a's content (moved).
    store
        .conn
        .execute(
            "UPDATE items SET meta = ?2 WHERE id = ?1",
            rusqlite::params![a.0.cast_signed(), meta_of(b).1],
        )
        .unwrap();
    store
        .conn
        .execute(
            "UPDATE items SET content = ?2 WHERE id = ?1",
            rusqlite::params![c.0.cast_signed(), meta_of(a).2],
        )
        .unwrap();
    drop(store);

    let (store, restored, unreadable) = fx.reload(Settings::default(), SECOND);
    assert_eq!(unreadable, 2);
    assert_eq!(texts(&restored), ["d", "b"]);
    assert_eq!(rows(&store).len(), 2);
}

#[test]
fn destroy_removes_every_file() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("a"), fx.now);
    fx.sync(&mut store, &history);
    store.destroy().unwrap();
    assert!(files_in(fx.dir.path()).is_empty());
}

#[test]
fn opening_where_no_file_can_be_created_fails() {
    let fx = Fixture::new();
    let missing = fx.dir.path().join("missing").join("history.db");
    let err = Store::open(&missing, fx.key()).err().unwrap();
    assert!(matches!(err, StoreError::Io(_)));
    assert!(err.to_string().starts_with("History file error: "));
    assert!(err.source().is_some());
}

#[test]
fn store_errors_describe_themselves() {
    let crypto = StoreError::from(CryptoError::Random);
    assert_eq!(
        crypto.to_string(),
        "History encryption error: the system random number generator failed"
    );
    assert!(crypto.source().is_some());

    let database = StoreError::from(rusqlite::Error::InvalidQuery);
    assert!(database.to_string().starts_with("History database error: "));
    assert!(database.source().is_some());
}

#[test]
fn wall_clock_conversions() {
    let now = Instant::now();
    let wall = UNIX_EPOCH + Duration::from_secs(1_000);

    assert_eq!(wall_ms(now, now, wall), 1_000_000);
    assert_eq!(
        wall_ms(now.checked_sub(SECOND).unwrap(), now, wall),
        999_000
    );
    // An Instant after `now` counts as now.
    assert_eq!(wall_ms(now + SECOND, now, wall), 1_000_000);
    // Before the epoch.
    assert_eq!(wall_ms(now, now + Duration::from_secs(2_000), wall), 0);

    let earlier = now.checked_sub(10 * SECOND).unwrap();
    assert_eq!(instant_at(990_000, now, wall), earlier);
    assert_eq!(instant_at(1_000_000, now, wall), now);
    // In the future, and beyond what SystemTime can hold.
    assert_eq!(instant_at(2_000_000, now, wall), now);
    assert_eq!(instant_at(u64::MAX, now, wall), now);
}

#[test]
fn the_size_limit_is_eight_mebibytes() {
    assert_eq!(MAX_PERSISTED_BYTES, 8 * 1024 * 1024);
}

#[test]
fn a_busy_database_is_an_error_not_a_wipe() {
    let fx = Fixture::new();
    let (mut store, _) = fx.open();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text("a"), fx.now);
    fx.sync(&mut store, &history);
    drop(store);

    let other = rusqlite::Connection::open(fx.path()).unwrap();
    other.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let err = Store::open(&fx.path(), fx.key()).err().unwrap();
    assert!(matches!(err, StoreError::Database(_)));
    drop(other);

    let (_, restored, _) = fx.reload(Settings::default(), SECOND);
    assert_eq!(texts(&restored), ["a"]);
}

#[test]
fn erasing_reports_files_it_cannot_remove() {
    let fx = Fixture::new();
    assert!(erase_files(fx.dir.path()).is_err());
    assert!(erase_files(&fx.path()).is_ok());
}
