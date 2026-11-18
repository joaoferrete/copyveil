//! End-to-end tests of `Settings1`: limits, persistence and memory-only.

#![expect(clippy::unwrap_used, reason = "a failing test should panic")]

mod common;

use common::{OwnedStorage, TestStorage, daemon_with, error_name, memory_daemon};
use copyveil_core::Settings;
use copyveil_daemon::config::Config;
use copyveil_daemon::state::{Notice, State, Storage};
use futures_util::StreamExt;
use std::sync::atomic::Ordering;
use std::time::{Instant, SystemTime};

const AWS: &str = "AKIAWBHK8C2D086BCGWN";

#[tokio::test]
async fn limits_and_detectors_are_published() {
    let d = memory_daemon(Settings::default()).await;
    assert_eq!(
        d.settings.limits().await.unwrap(),
        (10, 300, 60, 3600, 50, 5000)
    );
    let detectors = d.settings.detectors().await.unwrap();
    assert_eq!(detectors.len(), 7);
    assert_eq!(
        detectors[0],
        (
            "github-token".to_owned(),
            "GitHub tokens".to_owned(),
            "Looks like a GitHub token".to_owned()
        )
    );
    assert_eq!(d.settings.secret_expiry().await.unwrap(), 30);
    assert_eq!(d.settings.suspected_expiry().await.unwrap(), 900);
    assert_eq!(d.settings.max_plain_items().await.unwrap(), 500);
    assert!(d.settings.disabled_detectors().await.unwrap().is_empty());
    assert!(!d.settings.memory_only().await.unwrap());
}

#[tokio::test]
async fn valid_changes_apply_and_are_saved() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("settings.toml");
    let d = daemon_with(
        State::load(
            Settings::default(),
            Storage::Memory,
            Notice::None,
            Instant::now(),
        ),
        Config::default(),
        Some(path.clone()),
        Box::new(common::NoStorage),
    )
    .await;

    d.settings.set_secret_expiry(45).await.unwrap();
    d.settings.set_suspected_expiry(1200).await.unwrap();
    d.settings.set_max_plain_items(1000).await.unwrap();
    d.settings
        .set_disabled_detectors(vec!["aws-access-key".into()])
        .await
        .unwrap();

    let saved = Config::load(&path);
    assert_eq!(saved.settings.secret_expiry.as_secs(), 45);
    assert_eq!(saved.settings.suspected_expiry.as_secs(), 1200);
    assert_eq!(saved.settings.max_plain_items, 1000);
    assert_eq!(
        d.settings.disabled_detectors().await.unwrap(),
        ["aws-access-key"]
    );

    // A disabled detector no longer flags; the new Secret expiry applies.
    let aws = d
        .capture
        .capture("text/plain", AWS.as_bytes().to_vec(), Vec::new(), false)
        .await
        .unwrap();
    let secret = d
        .capture
        .capture("text/plain", b"hunter2".to_vec(), Vec::new(), true)
        .await
        .unwrap();
    let list = d.history.list().await.unwrap();
    let find = |id| list.iter().find(|item| item.id == id).unwrap();
    assert_eq!(find(aws).sensitivity, "plain");
    assert!((44_000..=45_000).contains(&find(secret).expires_in_ms));
}

#[tokio::test]
async fn values_outside_the_limits_are_refused_and_change_nothing() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("settings.toml");
    let d = daemon_with(
        State::load(
            Settings::default(),
            Storage::Memory,
            Notice::None,
            Instant::now(),
        ),
        Config::default(),
        Some(path.clone()),
        Box::new(common::NoStorage),
    )
    .await;

    for err in [
        d.settings.set_secret_expiry(9).await.unwrap_err(),
        d.settings.set_secret_expiry(301).await.unwrap_err(),
        d.settings.set_suspected_expiry(59).await.unwrap_err(),
        d.settings.set_suspected_expiry(3601).await.unwrap_err(),
        d.settings.set_max_plain_items(49).await.unwrap_err(),
        d.settings.set_max_plain_items(5001).await.unwrap_err(),
        d.settings
            .set_disabled_detectors(vec!["retired".into()])
            .await
            .unwrap_err(),
    ] {
        assert_eq!(
            error_name(&err),
            "fdo:org.freedesktop.DBus.Error.InvalidArgs"
        );
    }
    assert_eq!(d.settings.secret_expiry().await.unwrap(), 30);
    assert_eq!(d.settings.max_plain_items().await.unwrap(), 500);
    assert!(!path.exists());
}

#[tokio::test]
async fn a_lower_limit_removes_items_and_announces_it() {
    let d = memory_daemon(Settings::default()).await;
    for n in 0..60 {
        d.capture
            .capture(
                "text/plain",
                format!("item {n}").into_bytes(),
                Vec::new(),
                false,
            )
            .await
            .unwrap();
    }
    let mut changed = d.history.receive_changed().await.unwrap();
    d.settings.set_max_plain_items(50).await.unwrap();
    changed.next().await.unwrap();
    assert_eq!(d.history.list().await.unwrap().len(), 50);
}

#[tokio::test]
async fn memory_only_erases_the_store_and_its_key_then_starts_fresh() {
    let dir = tempfile::TempDir::new().unwrap();
    let storage = TestStorage::new(dir.path());
    let (disk, notice) = {
        let opener = OwnedStorage(std::sync::Arc::clone(&storage));
        copyveil_daemon::StorageOpener::open(&opener).await
    };
    let d = daemon_with(
        State::load(Settings::default(), disk, notice, Instant::now()),
        Config::default(),
        Some(dir.path().join("settings.toml")),
        Box::new(OwnedStorage(std::sync::Arc::clone(&storage))),
    )
    .await;
    let kept = d
        .capture
        .capture("text/plain", b"keep me".to_vec(), Vec::new(), false)
        .await
        .unwrap();
    assert!(storage.path.exists());

    d.settings.set_memory_only(true).await.unwrap();
    assert!(!storage.path.exists());
    assert!(storage.forgot_key.load(Ordering::SeqCst));
    assert!(!d.history.on_disk().await.unwrap());
    assert_eq!(d.history.list().await.unwrap()[0].id, kept);
    assert!(Config::load(&dir.path().join("settings.toml")).memory_only);
    // Setting the same value again is a no-op.
    d.settings.set_memory_only(true).await.unwrap();

    d.settings.set_memory_only(false).await.unwrap();
    assert!(storage.path.exists());
    assert!(d.history.on_disk().await.unwrap());
    assert!(!Config::load(&dir.path().join("settings.toml")).memory_only);
    drop(d);

    // The fresh store mirrors the History that was in memory.
    let opener = OwnedStorage(std::sync::Arc::clone(&storage));
    let (reopened, _) = copyveil_daemon::StorageOpener::open(&opener).await;
    let Storage::Disk(mut store) = reopened else {
        panic!("expected a store on disk");
    };
    let loaded = store
        .load(Settings::default(), Instant::now(), SystemTime::now())
        .unwrap();
    assert_eq!(loaded.history.items().len(), 1);
    assert_eq!(loaded.history.items()[0].id().value(), kept);
}
