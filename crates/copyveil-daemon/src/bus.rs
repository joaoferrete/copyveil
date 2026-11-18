//! The D-Bus service: `History1` for the app, `Capture1` for capture
//! sources, `Settings1` for the settings window.
//!
//! All three live at [`OBJECT_PATH`] on the session bus under [`BUS_NAME`].
//! Clients use the proxies in `copyveil-dbus`, which also holds the names
//! and the Item summary, so both sides share one contract. Signals carry
//! Item ids only. Content leaves the daemon only as the reply to `Content`,
//! for one Item the caller names.

use crate::StorageOpener;
use crate::config::Config;
use crate::state::State;
use crate::wire;
use copyveil_core::settings::{MAX_PLAIN_ITEMS, SECRET_EXPIRY, SUSPECTED_EXPIRY};
use copyveil_core::{Capture, Content, HistoryError, ItemId, Settings};
use copyveil_dbus::ItemSummary;
use copyveil_detect::Detector;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify};
use zbus::object_server::SignalEmitter;
use zbus::{Connection, interface};

pub use copyveil_dbus::{BUS_NAME, OBJECT_PATH};

/// Errors returned over D-Bus. Messages name Item ids, never content.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "io.github.joaoferrete.Copyveil.Error")]
pub enum Error {
    /// A zbus failure.
    #[zbus(error)]
    ZBus(zbus::Error),
    /// No Item has this id; it may have expired.
    UnknownItem(String),
    /// Only Plain Items can be pinned.
    NotPlain(String),
    /// The Item isn't pinned.
    NotPinned(String),
    /// There is no such Pin slot.
    NoSuchPinSlot(String),
    /// An argument was malformed.
    InvalidArgument(String),
}

impl From<HistoryError> for Error {
    fn from(err: HistoryError) -> Self {
        let message = err.to_string();
        match err {
            HistoryError::UnknownItem(_) => Self::UnknownItem(message),
            HistoryError::NotPlain(_) => Self::NotPlain(message),
            HistoryError::NotPinned(_) => Self::NotPinned(message),
            HistoryError::NoSuchPinSlot(_) => Self::NoSuchPinSlot(message),
        }
    }
}

/// State shared by both interfaces and the Expiry task.
pub struct Shared {
    state: Mutex<State>,
    /// Woken after every change, so the Expiry task can recompute its timer.
    changed: Notify,
    /// The settings as last saved. Lock before `state` when holding both.
    config: Mutex<Config>,
    config_path: Option<PathBuf>,
    storage: Box<dyn StorageOpener>,
}

impl Shared {
    /// Wraps the daemon state, its settings and where its storage comes from.
    #[must_use]
    pub fn new(
        state: State,
        config: Config,
        config_path: Option<PathBuf>,
        storage: Box<dyn StorageOpener>,
    ) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(state),
            changed: Notify::new(),
            config: Mutex::new(config),
            config_path,
            storage,
        })
    }

    /// Validates `change` applied to the current settings, then applies and
    /// saves them. Returns the error message for D-Bus on refusal.
    async fn change_settings(
        &self,
        change: impl FnOnce(&mut Settings),
    ) -> Result<(), zbus::fdo::Error> {
        let mut config = self.config.lock().await;
        let mut settings = config.settings.clone();
        change(&mut settings);
        settings
            .validate()
            .map_err(|err| zbus::fdo::Error::InvalidArgs(err.to_string()))?;
        self.state
            .lock()
            .await
            .set_settings(settings.clone(), Instant::now());
        config.settings = settings;
        self.save(&config);
        self.changed.notify_one();
        Ok(())
    }

    fn save(&self, config: &Config) {
        if let Some(path) = &self.config_path
            && let Err(err) = config.save(path)
        {
            tracing::error!(%err, "could not save the settings");
        }
    }
}

/// `io.github.joaoferrete.Copyveil.History1`: what the app sees and does.
pub struct HistoryService {
    shared: Arc<Shared>,
}

/// `io.github.joaoferrete.Copyveil.Capture1`: what capture sources call.
pub struct CaptureService {
    shared: Arc<Shared>,
}

impl HistoryService {
    async fn change<T>(
        &self,
        emitter: &SignalEmitter<'_>,
        apply: impl FnOnce(&mut State, Instant) -> Result<T, HistoryError>,
    ) -> Result<T, Error> {
        let result = {
            let mut state = self.shared.state.lock().await;
            apply(&mut state, Instant::now())?
        };
        self.shared.changed.notify_one();
        Self::changed(emitter).await?;
        Ok(result)
    }
}

#[interface(name = "io.github.joaoferrete.Copyveil.History1")]
impl HistoryService {
    /// Every Item, newest first, without the content of masked Items.
    async fn list(&self) -> Vec<ItemSummary> {
        let state = self.shared.state.lock().await;
        wire::summaries(state.history(), Instant::now())
    }

    /// The first and last four characters of each Suspected text Item of at
    /// least 20 characters, by id, so the popup can show which token it is.
    /// Never for Secret Items. See `copyveil_core::Item::suspected_ends`.
    async fn suspected_ends(&self) -> std::collections::HashMap<u64, (String, String)> {
        let state = self.shared.state.lock().await;
        state
            .history()
            .items()
            .iter()
            .filter_map(|item| Some((item.id().value(), item.suspected_ends()?)))
            .collect()
    }

    /// Ids of the Items matching `query`, newest first. Masked Items match
    /// only on their description.
    async fn search(&self, query: &str) -> Vec<u64> {
        let state = self.shared.state.lock().await;
        state
            .history()
            .search(query)
            .into_iter()
            .map(ItemId::value)
            .collect()
    }

    /// The content of one Item and its plain-text fallback (empty if none),
    /// for a Reveal or a paste.
    async fn content(&self, id: u64) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let state = self.shared.state.lock().await;
        let id = ItemId::from_value(id);
        let item = state
            .history()
            .get(id)
            .ok_or(HistoryError::UnknownItem(id))?;
        let fallback = item
            .plain_fallback()
            .map(Content::expose)
            .unwrap_or_default();
        Ok((item.content().expose().to_vec(), fallback.to_vec()))
    }

    /// The id of the Item in Pin slot `slot`, counting from 1.
    async fn pin_slot(&self, slot: u32) -> Result<u64, Error> {
        let state = self.shared.state.lock().await;
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        state
            .history()
            .pin_slot(slot)
            .map(|item| item.id().value())
            .ok_or_else(|| HistoryError::NoSuchPinSlot(slot).into())
    }

    /// Asks the capture source to paste an Item.
    async fn paste(
        &self,
        #[zbus(connection)] connection: &Connection,
        id: u64,
    ) -> Result<(), Error> {
        {
            let state = self.shared.state.lock().await;
            let id = ItemId::from_value(id);
            state
                .history()
                .get(id)
                .ok_or(HistoryError::UnknownItem(id))?;
        }
        let emitter = SignalEmitter::new(connection, OBJECT_PATH)?;
        CaptureService::paste_requested(&emitter, id).await?;
        Ok(())
    }

    /// Removes an Item.
    async fn remove(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        id: u64,
    ) -> Result<(), Error> {
        self.change(&emitter, |state, now| {
            state.remove(ItemId::from_value(id), now)
        })
        .await
    }

    /// Pins a Plain Item into the next free Pin slot.
    async fn pin(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        id: u64,
    ) -> Result<(), Error> {
        self.change(&emitter, |state, now| {
            state.pin(ItemId::from_value(id), now)
        })
        .await
    }

    /// Unpins an Item.
    async fn unpin(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        id: u64,
    ) -> Result<(), Error> {
        self.change(&emitter, |state, now| {
            state.unpin(ItemId::from_value(id), now)
        })
        .await
    }

    /// Moves a pinned Item to Pin slot `slot`, counting from 1.
    async fn move_pin(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        id: u64,
        slot: u32,
    ) -> Result<(), Error> {
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        self.change(&emitter, |state, now| {
            state.move_pin(ItemId::from_value(id), slot, now)
        })
        .await
    }

    /// Changes an Item's Sensitivity: `plain`, `suspected` or `secret`.
    async fn r#override(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        id: u64,
        sensitivity: &str,
    ) -> Result<(), Error> {
        let sensitivity = wire::parse_sensitivity(sensitivity).ok_or_else(|| {
            Error::InvalidArgument("sensitivity must be plain, suspected or secret".into())
        })?;
        self.change(&emitter, |state, now| {
            state.override_sensitivity(ItemId::from_value(id), sensitivity, now)
        })
        .await
    }

    /// Whether Plain Items are kept on disk (`true`) or in memory only.
    #[zbus(property)]
    async fn on_disk(&self) -> bool {
        self.shared.state.lock().await.on_disk()
    }

    /// What the user should be told about storage: empty, or one of
    /// `keyring-unavailable`, `store-unavailable`, `store-wiped`,
    /// `items-unreadable`, `sync-failed`.
    #[zbus(property)]
    async fn storage_notice(&self) -> String {
        self.shared.state.lock().await.notice().name().to_owned()
    }

    /// The History changed: list it again.
    #[zbus(signal)]
    async fn changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

#[interface(name = "io.github.joaoferrete.Copyveil.Capture1")]
impl CaptureService {
    /// Reports a clipboard change and returns the Item's id.
    ///
    /// `mime` is the type the source chose (`text/plain`, `text/html`,
    /// `image/png`…); `plain_fallback` is the text version of rich text, or
    /// empty. `password_manager_hint` is true when the clipboard offered
    /// `x-kde-passwordManagerHint: secret`.
    async fn capture(
        &self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        mime: &str,
        content: Vec<u8>,
        plain_fallback: Vec<u8>,
        password_manager_hint: bool,
    ) -> Result<u64, Error> {
        let content = Content::new(content);
        let plain_fallback = Content::new(plain_fallback);
        let kind = wire::kind_for_mime(mime)
            .ok_or_else(|| Error::InvalidArgument("unsupported MIME type".into()))?;
        if content.is_empty() {
            return Err(Error::InvalidArgument("empty content".into()));
        }
        let capture = Capture {
            content,
            kind,
            plain_fallback: (!plain_fallback.is_empty()).then_some(plain_fallback),
            password_manager_hint,
        };
        let id = {
            let mut state = self.shared.state.lock().await;
            state.capture(capture, Instant::now())
        };
        self.shared.changed.notify_one();
        HistoryService::changed(&emitter).await?;
        Ok(id.value())
    }

    /// These Items reached their Expiry and are gone. A source that last
    /// put one of them on the clipboard clears the clipboard.
    #[zbus(signal)]
    async fn expired(emitter: &SignalEmitter<'_>, ids: Vec<u64>) -> zbus::Result<()>;

    /// The user asked to paste this Item: fetch it with `History1.Content`,
    /// put it on the clipboard and paste it.
    #[zbus(signal)]
    async fn paste_requested(emitter: &SignalEmitter<'_>, id: u64) -> zbus::Result<()>;
}

/// Builds both interfaces over `shared`.
#[must_use]
pub fn services(shared: &Arc<Shared>) -> (HistoryService, CaptureService, SettingsService) {
    (
        HistoryService {
            shared: Arc::clone(shared),
        },
        CaptureService {
            shared: Arc::clone(shared),
        },
        SettingsService {
            shared: Arc::clone(shared),
        },
    )
}

/// `io.github.joaoferrete.Copyveil.Settings1`: what the settings window changes.
pub struct SettingsService {
    shared: Arc<Shared>,
}

/// Emits `PropertiesChanged` for History1's `OnDisk` and `StorageNotice`.
async fn storage_changed(server: &zbus::ObjectServer) -> Result<(), zbus::fdo::Error> {
    let failed = |err: zbus::Error| zbus::fdo::Error::Failed(err.to_string());
    let history = server
        .interface::<_, HistoryService>(OBJECT_PATH)
        .await
        .map_err(failed)?;
    let emitter = history.signal_emitter();
    history
        .get()
        .await
        .on_disk_changed(emitter)
        .await
        .map_err(failed)?;
    history
        .get()
        .await
        .storage_notice_changed(emitter)
        .await
        .map_err(failed)
}

/// Tells History1 clients to list again: settings can remove Items (a lower
/// limit) or change where they're kept (`OnDisk`, `StorageNotice`).
async fn announce(emitter: &SignalEmitter<'_>) -> Result<(), zbus::fdo::Error> {
    HistoryService::changed(emitter)
        .await
        .map_err(|err| zbus::fdo::Error::Failed(err.to_string()))
}

fn seconds(duration: Duration) -> u32 {
    u32::try_from(duration.as_secs()).unwrap_or(u32::MAX)
}

#[interface(name = "io.github.joaoferrete.Copyveil.Settings1")]
impl SettingsService {
    /// The limits every setting must stay within:
    /// `(secret_min_s, secret_max_s, suspected_min_s, suspected_max_s, items_min, items_max)`.
    #[expect(
        clippy::unused_self,
        reason = "D-Bus methods are called on the interface object"
    )]
    fn limits(&self) -> (u32, u32, u32, u32, u32, u32) {
        let items = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        (
            seconds(*SECRET_EXPIRY.start()),
            seconds(*SECRET_EXPIRY.end()),
            seconds(*SUSPECTED_EXPIRY.start()),
            seconds(*SUSPECTED_EXPIRY.end()),
            items(*MAX_PLAIN_ITEMS.start()),
            items(*MAX_PLAIN_ITEMS.end()),
        )
    }

    /// Every detector: `(name, title, description)`, in a stable order.
    #[expect(
        clippy::unused_self,
        reason = "D-Bus methods are called on the interface object"
    )]
    fn detectors(&self) -> Vec<(String, String, String)> {
        Detector::ALL
            .iter()
            .map(|detector| {
                (
                    detector.name().to_owned(),
                    detector.title().to_owned(),
                    detector.description().to_owned(),
                )
            })
            .collect()
    }

    /// Seconds a Secret lives before its Expiry.
    #[zbus(property)]
    async fn secret_expiry(&self) -> u32 {
        seconds(self.shared.config.lock().await.settings.secret_expiry)
    }

    #[zbus(property)]
    async fn set_secret_expiry(
        &self,
        value: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), zbus::fdo::Error> {
        self.shared
            .change_settings(|s| s.secret_expiry = Duration::from_secs(value.into()))
            .await?;
        announce(&emitter).await
    }

    /// Seconds a Suspected Item lives before its Expiry.
    #[zbus(property)]
    async fn suspected_expiry(&self) -> u32 {
        seconds(self.shared.config.lock().await.settings.suspected_expiry)
    }

    #[zbus(property)]
    async fn set_suspected_expiry(
        &self,
        value: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), zbus::fdo::Error> {
        self.shared
            .change_settings(|s| s.suspected_expiry = Duration::from_secs(value.into()))
            .await?;
        announce(&emitter).await
    }

    /// How many unpinned Plain Items the History keeps. Pins don't count.
    #[zbus(property)]
    async fn max_plain_items(&self) -> u32 {
        let max = self.shared.config.lock().await.settings.max_plain_items;
        u32::try_from(max).unwrap_or(u32::MAX)
    }

    #[zbus(property)]
    async fn set_max_plain_items(
        &self,
        value: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), zbus::fdo::Error> {
        let value = usize::try_from(value).unwrap_or(usize::MAX);
        self.shared
            .change_settings(|s| s.max_plain_items = value)
            .await?;
        announce(&emitter).await
    }

    /// Names of the detectors turned off (see `Detectors`).
    #[zbus(property)]
    async fn disabled_detectors(&self) -> Vec<String> {
        let detectors = self.shared.config.lock().await.settings.detectors;
        Detector::ALL
            .iter()
            .filter(|detector| !detectors.contains(**detector))
            .map(|detector| detector.name().to_owned())
            .collect()
    }

    #[zbus(property)]
    async fn set_disabled_detectors(
        &self,
        value: Vec<String>,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> Result<(), zbus::fdo::Error> {
        let mut disabled = Vec::new();
        for name in &value {
            let detector = Detector::from_name(name)
                .ok_or_else(|| zbus::fdo::Error::InvalidArgs("unknown detector name".to_owned()))?;
            disabled.push(detector);
        }
        self.shared
            .change_settings(|s| {
                s.detectors = disabled
                    .iter()
                    .fold(copyveil_detect::DetectorSet::all(), |set, d| {
                        set.without(*d)
                    });
            })
            .await?;
        announce(&emitter).await
    }

    /// Whether the History is kept in memory only. Turning it on erases the
    /// saved History and forgets its key; turning it off starts a new,
    /// empty store and writes the Plain Items in memory to it.
    #[zbus(property)]
    async fn memory_only(&self) -> bool {
        self.shared.config.lock().await.memory_only
    }

    #[zbus(property)]
    async fn set_memory_only(
        &self,
        value: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
        #[zbus(object_server)] server: &zbus::ObjectServer,
    ) -> Result<(), zbus::fdo::Error> {
        let mut config = self.shared.config.lock().await;
        if config.memory_only == value {
            return Ok(());
        }
        if value {
            self.shared.state.lock().await.go_memory_only();
            self.shared.storage.forget_key().await;
        } else {
            let (storage, notice) = self.shared.storage.open_fresh().await;
            self.shared
                .state
                .lock()
                .await
                .attach(storage, notice, Instant::now());
        }
        config.memory_only = value;
        self.shared.save(&config);
        self.shared.changed.notify_one();
        drop(config);
        storage_changed(server).await?;
        announce(&emitter).await
    }
}

/// Runs Expiry forever: sleeps until the next Expiry or the next change,
/// erases what is due and announces it.
///
/// # Errors
///
/// Returns when a signal can't be emitted (the connection is gone).
pub async fn run_expiry(shared: Arc<Shared>, connection: Connection) -> zbus::Result<()> {
    let emitter = SignalEmitter::new(&connection, OBJECT_PATH)?;
    loop {
        let next = shared.state.lock().await.history().next_expiry();
        let changed = shared.changed.notified();
        let Some(at) = next else {
            changed.await;
            continue;
        };
        tokio::select! {
            () = tokio::time::sleep_until(at.into()) => {}
            () = changed => continue,
        }
        let expired = shared.state.lock().await.expire(Instant::now());
        if !expired.is_empty() {
            tracing::debug!(count = expired.len(), "Items reached their Expiry");
            let ids = expired.into_iter().map(ItemId::value).collect();
            CaptureService::expired(&emitter, ids).await?;
            HistoryService::changed(&emitter).await?;
        }
    }
}
