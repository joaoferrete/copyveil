//! The app's link to the daemon.
//!
//! zbus runs on a tokio runtime of its own, off the GTK thread. Every call
//! is spawned there and awaited from the GTK main loop through its
//! `JoinHandle`, so no tokio I/O is ever polled by GTK. A failed call
//! becomes `None`: the UI shows "Copyveil isn't running" and carries on.

use copyveil_dbus::{HistoryProxy, ItemSummary};
use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

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

/// A cheap handle to the daemon's `History1`, cloned into callbacks.
#[derive(Clone)]
pub struct Daemon {
    history: Option<HistoryProxy<'static>>,
}

impl Daemon {
    /// Connects to the session bus. When that fails, every call returns `None`.
    #[must_use]
    pub fn connect() -> Self {
        let history = runtime().and_then(|runtime| {
            runtime.block_on(async {
                let connection = zbus::Connection::session().await.ok()?;
                HistoryProxy::new(&connection).await.ok()
            })
        });
        Self { history }
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
