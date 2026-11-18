//! End-to-end tests of the D-Bus service over a peer-to-peer connection.

#![allow(clippy::unwrap_used, reason = "a failing test should panic")]

use copyveil_core::{Key, Settings, Store};
use copyveil_daemon::bus::{self, CaptureServiceProxy, HistoryServiceProxy, OBJECT_PATH, Shared};
use copyveil_daemon::state::{Notice, State, Storage};
use futures_util::StreamExt;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::{Duration, Instant};
use zbus::Connection;
use zbus::connection::Builder;

const AWS: &str = "AKIAWBHK8C2D086BCGWN";

struct Daemon {
    shared: Arc<Shared>,
    server: Connection,
    history: HistoryServiceProxy<'static>,
    capture: CaptureServiceProxy<'static>,
}

async fn daemon(state: State) -> Daemon {
    let shared = Shared::new(state);
    let (history, capture) = bus::services(&shared);
    let (p0, p1) = UnixStream::pair().unwrap();
    let server = Builder::unix_stream(p0)
        .server(zbus::Guid::generate())
        .p2p()
        .serve_at(OBJECT_PATH, history)
        .serve_at(OBJECT_PATH, capture)
        .build();
    let client = Builder::unix_stream(p1).p2p().build();
    let (server, client) = tokio::try_join!(server, client).unwrap();
    Daemon {
        shared,
        server,
        history: HistoryServiceProxy::new(&client).await.unwrap(),
        capture: CaptureServiceProxy::new(&client).await.unwrap(),
    }
}

async fn memory_daemon(settings: Settings) -> Daemon {
    daemon(State::load(
        settings,
        Storage::Memory,
        Notice::None,
        Instant::now(),
    ))
    .await
}

fn error_name(err: &bus::Error) -> String {
    zbus::DBusError::name(err).to_string()
}

#[tokio::test]
async fn a_capture_is_listed_and_its_content_returned() {
    let d = memory_daemon(Settings::default()).await;
    let mut changed = d.history.receive_changed().await.unwrap();

    let id = d
        .capture
        .capture(
            "text/plain;charset=utf-8",
            b"git status".to_vec(),
            Vec::new(),
            false,
        )
        .await
        .unwrap();
    changed.next().await.unwrap();

    let list = d.history.list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].kind, "text");
    assert_eq!(list[0].sensitivity, "plain");
    assert_eq!(list[0].preview, "git status");
    assert_eq!(
        d.history.content(id).await.unwrap(),
        (b"git status".to_vec(), Vec::new())
    );
}

#[tokio::test]
async fn masked_items_are_listed_without_content() {
    let d = memory_daemon(Settings::default()).await;
    let secret = d
        .capture
        .capture(
            "text/plain",
            b"COPYVEIL-MARKER-secret".to_vec(),
            Vec::new(),
            true,
        )
        .await
        .unwrap();
    let suspected = d
        .capture
        .capture("text/plain", AWS.as_bytes().to_vec(), Vec::new(), false)
        .await
        .unwrap();

    let list = d.history.list().await.unwrap();
    assert_eq!(list[0].id, suspected);
    assert_eq!(list[0].sensitivity, "suspected");
    assert_eq!(list[1].id, secret);
    assert_eq!(list[1].sensitivity, "secret");
    assert_eq!(list[1].reason, "From password manager");
    assert!(list.iter().all(|item| item.preview.is_empty()));
    assert!(list[1].expires_in_ms > 0);

    // Search never matches masked content, only the description.
    assert!(d.history.search("MARKER").await.unwrap().is_empty());
    assert_eq!(d.history.search("password").await.unwrap(), [secret]);

    // A Reveal asks for the one Item.
    assert_eq!(
        d.history.content(secret).await.unwrap().0,
        b"COPYVEIL-MARKER-secret"
    );
}

#[tokio::test]
async fn rich_text_keeps_its_fallback() {
    let d = memory_daemon(Settings::default()).await;
    let id = d
        .capture
        .capture("text/html", b"<b>hi</b>".to_vec(), b"hi".to_vec(), false)
        .await
        .unwrap();
    assert_eq!(
        d.history.content(id).await.unwrap(),
        (b"<b>hi</b>".to_vec(), b"hi".to_vec())
    );
    assert_eq!(d.history.list().await.unwrap()[0].preview, "hi");
}

#[tokio::test]
async fn malformed_captures_are_refused() {
    let d = memory_daemon(Settings::default()).await;
    let prefix = "io.github.joaoferrete.Copyveil.Error.";
    let err = d
        .capture
        .capture("text/uri-list", b"file:///x".to_vec(), Vec::new(), false)
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}InvalidArgument"));
    let err = d
        .capture
        .capture("text/plain", Vec::new(), Vec::new(), false)
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}InvalidArgument"));
    assert!(d.history.list().await.unwrap().is_empty());
}

#[tokio::test]
async fn pins_and_overrides() {
    let d = memory_daemon(Settings::default()).await;
    let prefix = "io.github.joaoferrete.Copyveil.Error.";
    let a = d
        .capture
        .capture("text/plain", b"a".to_vec(), Vec::new(), false)
        .await
        .unwrap();
    let b = d
        .capture
        .capture("text/plain", b"b".to_vec(), Vec::new(), false)
        .await
        .unwrap();

    d.history.pin(a).await.unwrap();
    d.history.pin(b).await.unwrap();
    assert_eq!(d.history.pin_slot(1).await.unwrap(), a);
    d.history.move_pin(b, 1).await.unwrap();
    assert_eq!(d.history.pin_slot(1).await.unwrap(), b);
    let list = d.history.list().await.unwrap();
    assert_eq!((list[0].id, list[0].pin_slot), (b, 1));
    assert_eq!((list[1].id, list[1].pin_slot), (a, 2));

    d.history.unpin(b).await.unwrap();
    assert_eq!(d.history.pin_slot(1).await.unwrap(), a);
    let err = d.history.pin_slot(2).await.unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}NoSuchPinSlot"));
    let err = d.history.move_pin(b, 1).await.unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}NotPinned"));

    d.history.r#override(a, "secret").await.unwrap();
    let list = d.history.list().await.unwrap();
    assert_eq!(list[1].sensitivity, "secret");
    assert_eq!(list[1].reason, "Changed by you");
    assert_eq!(list[1].pin_slot, 0);
    let err = d.history.pin(a).await.unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}NotPlain"));
    let err = d.history.r#override(a, "Secret").await.unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}InvalidArgument"));

    d.history.remove(a).await.unwrap();
    let err = d.history.remove(a).await.unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}UnknownItem"));
    let err = d.history.content(a).await.unwrap_err();
    assert_eq!(error_name(&err), format!("{prefix}UnknownItem"));
    assert_eq!(d.history.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn paste_asks_the_capture_source() {
    let d = memory_daemon(Settings::default()).await;
    let mut requests = d.capture.receive_paste_requested().await.unwrap();
    let id = d
        .capture
        .capture("text/plain", b"a".to_vec(), Vec::new(), false)
        .await
        .unwrap();
    d.history.paste(id).await.unwrap();
    let request = requests.next().await.unwrap();
    assert_eq!(request.args().unwrap().id, id);

    let err = d.history.paste(id + 1).await.unwrap_err();
    assert_eq!(
        error_name(&err),
        "io.github.joaoferrete.Copyveil.Error.UnknownItem"
    );
}

#[tokio::test]
async fn expiry_erases_and_announces() {
    let d = memory_daemon(Settings {
        secret_expiry: Duration::from_millis(150),
        ..Settings::default()
    })
    .await;
    let mut expired = d.capture.receive_expired().await.unwrap();
    let task = tokio::spawn(bus::run_expiry(Arc::clone(&d.shared), d.server.clone()));

    let plain = d
        .capture
        .capture("text/plain", b"stays".to_vec(), Vec::new(), false)
        .await
        .unwrap();
    let secret = d
        .capture
        .capture("text/plain", b"goes".to_vec(), Vec::new(), true)
        .await
        .unwrap();

    let signal = tokio::time::timeout(Duration::from_secs(5), expired.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(signal.args().unwrap().ids, [secret]);
    let ids: Vec<u64> = d
        .history
        .list()
        .await
        .unwrap()
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids, [plain]);
    task.abort();
}

#[tokio::test]
async fn storage_properties() {
    let d = daemon(State::load(
        Settings::default(),
        Storage::Memory,
        Notice::KeyringUnavailable,
        Instant::now(),
    ))
    .await;
    assert!(!d.history.on_disk().await.unwrap());
    assert_eq!(
        d.history.storage_notice().await.unwrap(),
        "keyring-unavailable"
    );
}

#[tokio::test]
async fn plain_items_outlive_the_daemon_and_secrets_do_not() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("history.db");
    let key = *Key::generate().unwrap().expose();
    let open = || {
        let (store, _) = Store::open(&path, Key::from_bytes(&key).unwrap()).unwrap();
        State::load(
            Settings::default(),
            Storage::Disk(Box::new(store)),
            Notice::None,
            Instant::now(),
        )
    };

    let d = daemon(open()).await;
    assert!(d.history.on_disk().await.unwrap());
    assert_eq!(d.history.storage_notice().await.unwrap(), "");
    let plain = d
        .capture
        .capture("text/plain", b"keep me".to_vec(), Vec::new(), false)
        .await
        .unwrap();
    d.history.pin(plain).await.unwrap();
    d.capture
        .capture("text/plain", b"forget me".to_vec(), Vec::new(), true)
        .await
        .unwrap();
    d.capture
        .capture("text/plain", AWS.as_bytes().to_vec(), Vec::new(), false)
        .await
        .unwrap();
    drop(d);

    let d = daemon(open()).await;
    let list = d.history.list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, plain);
    assert_eq!(list[0].preview, "keep me");
    assert_eq!(list[0].pin_slot, 1);
}
