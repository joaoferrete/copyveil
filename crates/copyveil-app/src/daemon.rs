//! The app's link to the daemon.
//!
//! zbus runs on a tokio runtime of its own, off the GTK thread. Every call
//! is spawned there and awaited from the GTK main loop through its
//! `JoinHandle`, so no tokio I/O is ever polled by GTK. A failed call
//! becomes `None`: the UI shows "Copyveil isn't running" and carries on.

use copyveil_dbus::{HistoryProxy, ItemSummary, SettingsProxy};
use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::Runtime;
use zbus::proxy::CacheProperties;

/// The Copyveil GNOME Shell extension.
pub const EXTENSION_UUID: &str = "copyveil@joaoferrete.github.io";

/// The runtime zbus runs on, or `None` if it couldn't be started (then
/// every call fails, as if the daemon were unreachable).
fn runtime() -> Option<&'static Runtime> {
    static RUNTIME: OnceLock<Option<Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name("copyveil-dbus")
                .build()
                .ok()
        })
        .as_ref()
}

/// A cheap handle to the daemon's `History1` and `Settings1`, cloned into callbacks.
#[derive(Clone)]
pub struct Daemon {
    history: Option<HistoryProxy<'static>>,
    settings: Option<SettingsProxy<'static>>,
}

/// The current settings, as the settings window shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsValues {
    /// Seconds a Secret lives.
    pub secret_expiry: u32,
    /// Seconds a Suspected Item lives.
    pub suspected_expiry: u32,
    /// Unpinned Plain Items kept.
    pub max_plain_items: u32,
    /// Names of the detectors turned off.
    pub disabled_detectors: Vec<String>,
    /// Whether the History is kept in memory only.
    pub memory_only: bool,
}

impl Daemon {
    /// Connects to the session bus. When that fails, every call returns `None`.
    ///
    /// Properties aren't cached: `OnDisk` and `StorageNotice` change when
    /// memory-only does, and the window must always show the current value.
    #[must_use]
    pub fn connect() -> Self {
        let proxies = runtime().and_then(|runtime| {
            runtime.block_on(async {
                let connection = zbus::Connection::session().await.ok()?;
                let history = HistoryProxy::builder(&connection)
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await
                    .ok()?;
                let settings = SettingsProxy::builder(&connection)
                    .cache_properties(CacheProperties::No)
                    .build()
                    .await
                    .ok()?;
                Some((history, settings))
            })
        });
        let (history, settings) = proxies.unzip();
        Self { history, settings }
    }

    async fn settings_call<T, F>(&self, call: impl FnOnce(SettingsProxy<'static>) -> F) -> Option<T>
    where
        T: Send + 'static,
        F: Future<Output = zbus::Result<T>> + Send + 'static,
    {
        let proxy = self.settings.clone()?;
        runtime()?.spawn(call(proxy)).await.ok()?.ok()
    }

    /// The limits for every setting: see `Settings1.Limits`.
    pub async fn limits(&self) -> Option<(u32, u32, u32, u32, u32, u32)> {
        self.settings_call(|s| async move { s.limits().await })
            .await
    }

    /// Every detector as `(name, title, description)`.
    pub async fn detectors(&self) -> Option<Vec<(String, String, String)>> {
        self.settings_call(|s| async move { s.detectors().await })
            .await
    }

    /// Every setting's current value.
    pub async fn settings(&self) -> Option<SettingsValues> {
        self.settings_call(|s| async move {
            Ok(SettingsValues {
                secret_expiry: s.secret_expiry().await?,
                suspected_expiry: s.suspected_expiry().await?,
                max_plain_items: s.max_plain_items().await?,
                disabled_detectors: s.disabled_detectors().await?,
                memory_only: s.memory_only().await?,
            })
        })
        .await
    }

    /// Changes one setting.
    ///
    /// # Errors
    ///
    /// The daemon's refusal (a value outside its limit) or "Copyveil isn’t
    /// running", as a message to show in a toast.
    pub async fn change(&self, change: SettingChange) -> Result<(), String> {
        let proxy = self
            .settings
            .clone()
            .ok_or_else(|| "Copyveil isn’t running".to_owned())?;
        let runtime = runtime().ok_or_else(|| "Copyveil isn’t running".to_owned())?;
        let result = runtime
            .spawn(async move {
                match change {
                    SettingChange::SecretExpiry(v) => proxy.set_secret_expiry(v).await,
                    SettingChange::SuspectedExpiry(v) => proxy.set_suspected_expiry(v).await,
                    SettingChange::MaxPlainItems(v) => proxy.set_max_plain_items(v).await,
                    SettingChange::DisabledDetectors(v) => proxy.set_disabled_detectors(v).await,
                    SettingChange::MemoryOnly(v) => proxy.set_memory_only(v).await,
                }
            })
            .await
            .map_err(|_| "Copyveil isn’t running".to_owned())?;
        result.map_err(|err| match err {
            zbus::Error::FDO(fdo) => match *fdo {
                zbus::fdo::Error::InvalidArgs(message) => message,
                other => other.to_string(),
            },
            other => other.to_string(),
        })
    }

    /// The Copyveil extension's state from GNOME Shell: `None` when GNOME
    /// Shell isn't reachable, `Some(None)` when the extension isn't
    /// installed, `Some(Some(state))` otherwise (1 is running).
    pub async fn extension_state(&self) -> Option<Option<f64>> {
        let connection = self.history.as_ref()?.inner().connection().clone();
        runtime()?
            .spawn(async move {
                let reply = connection
                    .call_method(
                        Some("org.gnome.Shell"),
                        "/org/gnome/Shell",
                        Some("org.gnome.Shell.Extensions"),
                        "GetExtensionInfo",
                        &(EXTENSION_UUID,),
                    )
                    .await
                    .ok()?;
                let mut info: std::collections::HashMap<String, zbus::OwnedVariant> =
                    reply.body().deserialize().ok()?;
                Some(
                    info.remove("state")
                        .and_then(|state| f64::try_from(state).ok()),
                )
            })
            .await
            .ok()?
    }

    /// Asks GNOME Shell to turn the Copyveil extension on.
    pub async fn enable_extension(&self) -> bool {
        let Some(connection) = self
            .history
            .as_ref()
            .map(|h| h.inner().connection().clone())
        else {
            return false;
        };
        let Some(runtime) = runtime() else {
            return false;
        };
        runtime
            .spawn(async move {
                connection
                    .call_method(
                        Some("org.gnome.Shell"),
                        "/org/gnome/Shell",
                        Some("org.gnome.Shell.Extensions"),
                        "EnableExtension",
                        &(EXTENSION_UUID,),
                    )
                    .await
                    .ok()
                    .and_then(|reply| reply.body().deserialize::<bool>().ok())
                    .unwrap_or(false)
            })
            .await
            .unwrap_or(false)
    }

    /// Changes an Item's Sensitivity (`plain`, `suspected` or `secret`).
    pub async fn override_sensitivity(&self, id: u64, sensitivity: &'static str) -> Option<()> {
        self.call(move |history| async move { history.r#override(id, sensitivity).await })
            .await
    }

    /// Moves a pinned Item to Pin slot `slot`.
    pub async fn move_pin(&self, id: u64, slot: u32) -> Option<()> {
        self.call(move |history| async move { history.move_pin(id, slot).await })
            .await
    }

    async fn call<T, F>(&self, call: impl FnOnce(HistoryProxy<'static>) -> F) -> Option<T>
    where
        T: Send + 'static,
        F: Future<Output = zbus::Result<T>> + Send + 'static,
    {
        let proxy = self.history.clone()?;
        runtime()?.spawn(call(proxy)).await.ok()?.ok()
    }

    /// Every Item, newest first, without masked content.
    pub async fn list(&self) -> Option<Vec<ItemSummary>> {
        self.call(|history| async move { history.list().await })
            .await
    }

    /// The ends of each Suspected Item that shows them, by id.
    pub async fn suspected_ends(&self) -> Option<std::collections::HashMap<u64, (String, String)>> {
        self.call(|history| async move { history.suspected_ends().await })
            .await
    }

    /// Ids matching `query`; masked Items match on their description only.
    pub async fn search(&self, query: String) -> Option<Vec<u64>> {
        self.call(|history| async move { history.search(&query).await })
            .await
    }

    /// One Item's content, for a Reveal. Returns the text to show, or `None`.
    pub async fn reveal(&self, id: u64) -> Option<String> {
        let (content, fallback) = self
            .call(move |history| async move { history.content(id).await })
            .await?;
        let text = if fallback.is_empty() {
            content
        } else {
            fallback
        };
        Some(String::from_utf8_lossy(&text).into_owned())
    }

    /// Asks the capture source to paste the Item.
    pub async fn paste(&self, id: u64) -> Option<()> {
        self.call(move |history| async move { history.paste(id).await })
            .await
    }

    /// Pins or unpins the Item.
    pub async fn set_pinned(&self, id: u64, pinned: bool) -> Option<()> {
        self.call(move |history| async move {
            if pinned {
                history.pin(id).await
            } else {
                history.unpin(id).await
            }
        })
        .await
    }

    /// Removes the Item.
    pub async fn remove(&self, id: u64) -> Option<()> {
        self.call(move |history| async move { history.remove(id).await })
            .await
    }

    /// Whether Plain Items are kept on disk, and the storage notice.
    pub async fn storage(&self) -> Option<(bool, String)> {
        self.call(|history| async move {
            Ok((history.on_disk().await?, history.storage_notice().await?))
        })
        .await
    }

    /// A channel that receives `()` each time the History changes.
    #[must_use]
    pub fn changes(&self) -> async_channel::Receiver<()> {
        let (sender, receiver) = async_channel::unbounded();
        if let (Some(history), Some(runtime)) = (self.history.clone(), runtime()) {
            runtime.spawn(async move {
                use futures_util::StreamExt;
                let Ok(mut changes) = history.receive_changed().await else {
                    return;
                };
                while changes.next().await.is_some() {
                    if sender.send(()).await.is_err() {
                        break;
                    }
                }
            });
        }
        receiver
    }
}

/// One setting to change over `Settings1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingChange {
    /// Seconds a Secret lives.
    SecretExpiry(u32),
    /// Seconds a Suspected Item lives.
    SuspectedExpiry(u32),
    /// Unpinned Plain Items kept.
    MaxPlainItems(u32),
    /// Names of the detectors turned off.
    DisabledDetectors(Vec<String>),
    /// Keep the History in memory only.
    MemoryOnly(bool),
}
