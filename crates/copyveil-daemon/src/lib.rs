//! The Copyveil daemon: keeps the History and serves it over D-Bus.
//!
//! At start-up it reads the History key from the keyring and opens the
//! encrypted store. Without a keyring it runs in memory only and says so
//! (ADR 0002). Capture sources report clipboard changes through `Capture1`;
//! the app lists and changes the History through `History1`.

pub mod bus;
pub mod state;
pub mod wire;

use crate::bus::{BUS_NAME, OBJECT_PATH, Shared};
use crate::state::{Notice, State, Storage};
use copyveil_core::{Settings, Store};
use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::time::Instant;

/// Makes this process non-dumpable before it holds any Item.
///
/// The kernel then writes no core dump of it, `/proc/<pid>/mem` is closed to
/// other processes, and processes running as the same user can't attach to
/// it with `ptrace`, so they can't read Secret Items out of its memory.
///
/// # Errors
///
/// The `prctl` call failed. The daemon refuses to start in that case.
pub fn harden() -> std::io::Result<()> {
    use rustix::process::{DumpableBehavior, set_dumpable_behavior};
    set_dumpable_behavior(DumpableBehavior::NotDumpable)?;
    Ok(())
}

/// Where the store lives: `$XDG_DATA_HOME/copyveil/history.db`, or
/// `~/.local/share/copyveil/history.db`. `None` if neither is set.
#[must_use]
pub fn store_path() -> Option<PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })?;
    Some(data_home.join("copyveil").join("history.db"))
}

/// Opens the encrypted store, or explains why the History stays in memory.
pub async fn open_storage() -> (Storage, Notice) {
    let key = match copyveil_keyring::history_key().await {
        Ok(key) => key,
        Err(err) => {
            tracing::warn!(%err, "no History key; keeping the History in memory only");
            return (Storage::Memory, Notice::KeyringUnavailable);
        }
    };
    let Some(path) = store_path() else {
        tracing::warn!("neither XDG_DATA_HOME nor HOME is set; keeping the History in memory only");
        return (Storage::Memory, Notice::StoreUnavailable);
    };
    if let Some(dir) = path.parent()
        && let Err(err) = DirBuilder::new().recursive(true).mode(0o700).create(dir)
    {
        tracing::error!(%err, "could not create the data directory");
        return (Storage::Memory, Notice::StoreUnavailable);
    }
    match Store::open(&path, key) {
        Ok((store, opened)) => (Storage::Disk(Box::new(store)), Notice::for_opened(opened)),
        Err(err) => {
            tracing::error!(%err, "could not open the History store");
            (Storage::Memory, Notice::StoreUnavailable)
        }
    }
}

/// Runs the daemon on the session bus until it is stopped.
///
/// # Errors
///
/// Fails when the session bus is unreachable or another daemon already
/// owns [`BUS_NAME`].
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (storage, notice) = open_storage().await;
    let state = State::load(Settings::default(), storage, notice, Instant::now());
    tracing::info!(
        on_disk = state.on_disk(),
        notice = state.notice().name(),
        "History ready"
    );
    let shared = Shared::new(state);
    let (history, capture) = bus::services(&shared);
    let connection = zbus::connection::Builder::session()
        .name(BUS_NAME)
        .serve_at(OBJECT_PATH, history)
        .serve_at(OBJECT_PATH, capture)
        .build()
        .await?;
    tokio::select! {
        result = bus::run_expiry(shared, connection) => result?,
        result = shutdown() => result?,
    }
    tracing::info!("stopping; the History in memory is erased");
    Ok(())
}

/// Waits for SIGTERM (systemd) or SIGINT (Ctrl+C).
async fn shutdown() -> std::io::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;
    tokio::select! {
        _ = term.recv() => {}
        _ = int.recv() => {}
    }
    Ok(())
}
