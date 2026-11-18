//! The D-Bus service: `History1` for the app, `Capture1` for capture sources.
//!
//! Both interfaces live at [`OBJECT_PATH`] on the session bus under
//! [`BUS_NAME`]. Signals carry Item ids only. Content leaves the daemon only
//! as the reply to `Content`, for one Item the caller names.

use crate::state::State;
use crate::wire::{self, ItemSummary};
use copyveil_core::{Capture, Content, HistoryError, ItemId};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, Notify};
use zbus::object_server::SignalEmitter;
use zbus::{Connection, interface};

/// The daemon's well-known name on the session bus.
pub const BUS_NAME: &str = "io.github.joaoferrete.Copyveil.Daemon";
/// The object that implements both interfaces.
pub const OBJECT_PATH: &str = "/io/github/joaoferrete/Copyveil/Daemon";

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
}

impl Shared {
    /// Wraps the daemon state.
    #[must_use]
    pub fn new(state: State) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(state),
            changed: Notify::new(),
        })
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

#[interface(
    name = "io.github.joaoferrete.Copyveil.History1",
    proxy(
        default_service = "io.github.joaoferrete.Copyveil.Daemon",
        default_path = "/io/github/joaoferrete/Copyveil/Daemon",
    )
)]
impl HistoryService {
    /// Every Item, newest first, without the content of masked Items.
    async fn list(&self) -> Vec<ItemSummary> {
        let state = self.shared.state.lock().await;
        wire::summaries(state.history(), Instant::now())
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

#[interface(
    name = "io.github.joaoferrete.Copyveil.Capture1",
    proxy(
        default_service = "io.github.joaoferrete.Copyveil.Daemon",
        default_path = "/io/github/joaoferrete/Copyveil/Daemon",
    )
)]
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
pub fn services(shared: &Arc<Shared>) -> (HistoryService, CaptureService) {
    (
        HistoryService {
            shared: Arc::clone(shared),
        },
        CaptureService {
            shared: Arc::clone(shared),
        },
    )
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
