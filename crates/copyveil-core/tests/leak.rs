//! The leak test from `docs/testing.md`: whatever happens in the History, no
//! marker value ever appears in any file the store writes, in clear.

#![allow(clippy::unwrap_used, reason = "a failing test should panic")]

use copyveil_core::{Capture, Content, History, Key, Kind, Sensitivity, Settings, Store};
use std::path::Path;
use std::time::{Instant, SystemTime};

/// A Secret, flagged by a password manager.
const SECRET: &str = "COPYVEIL-MARKER-secret-correct-horse";
/// Suspected: a GitHub token shape.
const SUSPECTED: &str = "ghp_COPYVEILMARKERsuspected0123456789ab";
/// Plain at first, then promoted to Secret by an Override.
const PROMOTED: &str = "COPYVEIL-MARKER-promoted";
/// Plain: written to disk, but only sealed.
const PLAIN: &str = "COPYVEIL-MARKER-plain";
/// Rich text whose plain-text fallback is the marker.
const FALLBACK: &str = "COPYVEIL-MARKER-fallback";
/// Plain and pinned.
const PINNED: &str = "COPYVEIL-MARKER-pinned";

const MARKERS: [&str; 6] = [SECRET, SUSPECTED, PROMOTED, PLAIN, FALLBACK, PINNED];

/// Every byte of every file under `dir`, recursively.
fn all_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(all_files(&path));
        } else {
            files.push((path.display().to_string(), std::fs::read(&path).unwrap()));
        }
    }
    files
}

fn assert_no_marker(dir: &Path, moment: &str) {
    let files = all_files(dir);
    assert!(!files.is_empty());
    for (name, bytes) in files {
        for (index, marker) in MARKERS.iter().enumerate() {
            let marker = marker.as_bytes();
            assert!(
                !bytes.windows(marker.len()).any(|window| window == marker),
                "marker #{index} found in {name} {moment}"
            );
        }
    }
}

#[test]
fn no_marker_ever_reaches_disk_in_clear() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let key = *Key::generate().unwrap().expose();
    let start = Instant::now();
    let wall = SystemTime::now();
    let settings = Settings::default();

    let (mut store, _) = Store::open(&path, Key::from_bytes(&key).unwrap()).unwrap();
    let mut history = History::new(settings.clone());

    let secret = history.capture(Capture::secret_text(SECRET), start);
    let suspected = history.capture(Capture::text(SUSPECTED), start);
    let promoted = history.capture(Capture::text(PROMOTED), start);
    history.capture(Capture::text(PLAIN), start);
    history.capture(
        Capture {
            content: Content::from("<p>rich</p>"),
            kind: Kind::RichText,
            plain_fallback: Some(Content::from(FALLBACK)),
            password_manager_hint: false,
        },
        start,
    );
    let pinned = history.capture(Capture::text(PINNED), start);
    history.pin(pinned).unwrap();
    assert_eq!(
        history.get(secret).unwrap().sensitivity(),
        Sensitivity::Secret
    );
    assert_eq!(
        history.get(suspected).unwrap().sensitivity(),
        Sensitivity::Suspected
    );
    store.sync(&history, start, wall).unwrap();
    assert_no_marker(dir.path(), "after the first sync");

    history
        .override_sensitivity(promoted, Sensitivity::Secret, start)
        .unwrap();
    store.sync(&history, start, wall).unwrap();
    assert_no_marker(dir.path(), "after a promotion");

    let later = start + settings.suspected_expiry;
    let expired = history.expire(later);
    assert_eq!(expired.len(), 3);
    store
        .sync(&history, later, wall + settings.suspected_expiry)
        .unwrap();
    drop(store);
    assert_no_marker(dir.path(), "after Expiry and close");

    let (mut store, _) = Store::open(&path, Key::from_bytes(&key).unwrap()).unwrap();
    let loaded = store.load(settings.clone(), later, wall).unwrap();
    let mut restored: Vec<Vec<u8>> = loaded
        .history
        .items()
        .iter()
        .map(|item| item.content().expose().to_vec())
        .collect();
    restored.sort();
    assert_eq!(
        restored,
        [
            b"<p>rich</p>".to_vec(),
            PINNED.as_bytes().to_vec(),
            PLAIN.as_bytes().to_vec(),
        ]
    );
    store.destroy().unwrap();
    assert!(all_files(dir.path()).is_empty());
}

#[test]
fn a_store_switched_to_memory_only_leaves_nothing_behind() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let (mut store, _) = Store::open(&path, Key::generate().unwrap()).unwrap();
    let mut history = History::new(Settings::default());
    history.capture(Capture::text(PLAIN), Instant::now());
    store
        .sync(&history, Instant::now(), SystemTime::now())
        .unwrap();
    store.destroy().unwrap();
    assert!(all_files(dir.path()).is_empty());
}
