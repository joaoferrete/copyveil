//! A daemon served over a peer-to-peer D-Bus connection, for the
//! end-to-end tests: no session bus and no keyring.

#![allow(dead_code, reason = "each test file uses part of the harness")]

use copyveil_core::{Key, Settings, Store};
use copyveil_daemon::bus::{self, OBJECT_PATH, Shared};
use copyveil_daemon::config::Config;
use copyveil_daemon::state::{Notice, State, Storage};
use copyveil_daemon::{BoxFuture, StorageOpener, open_store_at};
use copyveil_dbus::{CaptureProxy, ERROR_PREFIX, HistoryProxy, SettingsProxy};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use zbus::Connection;
use zbus::connection::Builder;

/// A store in a directory with a fixed key, standing in for the keyring.
pub(crate) struct TestStorage {
    pub(crate) path: PathBuf,
    key: [u8; 32],
    pub(crate) forgot_key: AtomicBool,
}

impl TestStorage {
    pub(crate) fn new(dir: &Path) -> Arc<Self> {
        Arc::new(Self {
            path: dir.join("history.db"),
            key: *Key::generate().unwrap().expose(),
            forgot_key: AtomicBool::new(false),
        })
    }

    fn open_now(&self) -> (Storage, Notice) {
        open_store_at(&self.path, Key::from_bytes(&self.key).unwrap())
    }
}

/// A [`TestStorage`] the daemon owns while the test keeps a handle to it.
pub(crate) struct OwnedStorage(pub(crate) Arc<TestStorage>);

impl StorageOpener for OwnedStorage {
    fn open(&self) -> BoxFuture<'_, (Storage, Notice)> {
        Box::pin(async move { self.0.open_now() })
    }

    fn open_fresh(&self) -> BoxFuture<'_, (Storage, Notice)> {
        self.erase();
        self.open()
    }

    fn forget_key(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move { self.0.forgot_key.store(true, Ordering::SeqCst) })
    }

    fn erase(&self) {
        Store::erase(&self.0.path).unwrap();
    }
}

/// Storage that is never used, for tests that keep the History in memory.
pub(crate) struct NoStorage;

impl StorageOpener for NoStorage {
    fn open(&self) -> BoxFuture<'_, (Storage, Notice)> {
        Box::pin(async { (Storage::Memory, Notice::None) })
    }
    fn open_fresh(&self) -> BoxFuture<'_, (Storage, Notice)> {
        self.open()
    }
    fn forget_key(&self) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
    fn erase(&self) {}
}

pub(crate) struct Daemon {
    pub(crate) shared: Arc<Shared>,
    pub(crate) server: Connection,
    pub(crate) history: HistoryProxy<'static>,
    pub(crate) capture: CaptureProxy<'static>,
    pub(crate) settings: SettingsProxy<'static>,
}

pub(crate) async fn daemon_with(
    state: State,
    config: Config,
    config_path: Option<PathBuf>,
    storage: Box<dyn StorageOpener>,
) -> Daemon {
    let shared = Shared::new(state, config, config_path, storage);
    let (history, capture, settings) = bus::services(&shared);
    let (p0, p1) = UnixStream::pair().unwrap();
    let server = Builder::unix_stream(p0)
        .server(zbus::Guid::generate())
        .p2p()
        .serve_at(OBJECT_PATH, history)
        .serve_at(OBJECT_PATH, capture)
        .serve_at(OBJECT_PATH, settings)
        .build();
    let client = Builder::unix_stream(p1).p2p().build();
    let (server, client) = tokio::try_join!(server, client).unwrap();
    Daemon {
        shared,
        server,
        // Like the app: always read OnDisk and StorageNotice fresh.
        history: HistoryProxy::builder(&client)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .unwrap(),
        capture: CaptureProxy::new(&client).await.unwrap(),
        settings: SettingsProxy::builder(&client)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await
            .unwrap(),
    }
}

/// A daemon over `state`, with default settings and no storage behind it.
pub(crate) async fn daemon(state: State) -> Daemon {
    daemon_with(state, Config::default(), None, Box::new(NoStorage)).await
}

pub(crate) async fn memory_daemon(settings: Settings) -> Daemon {
    let config = Config {
        settings: settings.clone(),
        memory_only: false,
    };
    daemon_with(
        State::load(settings, Storage::Memory, Notice::None, Instant::now()),
        config,
        None,
        Box::new(NoStorage),
    )
    .await
}

/// The D-Bus error name, without the shared prefix.
pub(crate) fn error_name(err: &zbus::Error) -> String {
    match err {
        zbus::Error::MethodError(name, _, _) => name
            .strip_prefix(ERROR_PREFIX)
            .and_then(|rest| rest.strip_prefix('.'))
            .unwrap_or(name)
            .to_owned(),
        zbus::Error::FDO(fdo) => format!("fdo:{}", zbus::DBusError::name(fdo.as_ref())),
        other => format!("not a method error: {other}"),
    }
}
