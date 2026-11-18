//! The encrypted store: Plain Items on disk, so the History survives a restart.
//!
//! The store mirrors the in-memory [`History`]: the daemon calls
//! [`Store::sync`] after each change and [`Store::load`] once at start-up.
//! Only Plain Items up to [`MAX_PERSISTED_BYTES`] are written; Secret and
//! Suspected Items never are (ADR 0002). Each Item is one SQLite row whose
//! only clear column is its id; everything else is sealed with the at-rest
//! key (ADR 0005).

use crate::classify::classify;
use crate::crypto::{self, CryptoError, Key};
use crate::item::{Capture, Item, ItemId, Reason};
use crate::record::{Body, Meta};
use crate::{History, Sensitivity, Settings};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The largest Item, content and plain-text fallback together, that is
/// written to disk. Larger Items stay in the History until the daemon stops.
pub const MAX_PERSISTED_BYTES: usize = 8_388_608; // 8 MiB

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS key_check (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        sealed BLOB NOT NULL
    );
    CREATE TABLE IF NOT EXISTS items (
        id INTEGER PRIMARY KEY,
        meta BLOB NOT NULL,
        content BLOB NOT NULL
    );
";

const CHECK_AAD: &[u8] = b"copyveil/v1/key-check";
const CHECK_PLAINTEXT: &[u8] = b"copyveil history key check";

/// How [`Store::open`] found the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// There was no store; an empty one was created.
    Created,
    /// The store was there and the key opens it.
    Existing,
    /// The store was there but the key doesn't open it (the keyring entry
    /// was deleted or recreated), so it was erased and started empty. The
    /// app tells the user (ADR 0005).
    Wiped,
}

/// The History read back from disk.
pub struct Loaded {
    /// The restored History.
    pub history: History,
    /// How many stored Items could not be decrypted and were erased.
    pub unreadable: usize,
}

/// Why a store operation failed. Never carries Item content.
#[derive(Debug)]
pub enum StoreError {
    /// Creating or removing a store file failed.
    Io(io::Error),
    /// SQLite reported an error.
    Database(rusqlite::Error),
    /// Sealing a value failed.
    Crypto(CryptoError),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "History file error: {err}"),
            Self::Database(err) => write!(f, "History database error: {err}"),
            Self::Crypto(err) => write!(f, "History encryption error: {err}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Database(err) => Some(err),
            Self::Crypto(err) => Some(err),
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Database(err)
    }
}

impl From<CryptoError> for StoreError {
    fn from(err: CryptoError) -> Self {
        Self::Crypto(err)
    }
}

/// What a stored row says, as last written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row {
    order: u64,
    captured_at: Instant,
    pin_slot: u64,
    overridden: bool,
}

/// The encrypted store of Plain Items.
///
/// Row ids are Item ids, stored as SQLite's signed integers bit for bit.
pub struct Store {
    path: PathBuf,
    conn: Connection,
    key: Key,
    rows: HashMap<ItemId, Row>,
    next_order: u64,
}

impl Store {
    /// Opens the store at `path`, creating it readable by the user only.
    ///
    /// # Errors
    ///
    /// [`StoreError`] if the file can't be created or SQLite fails for a
    /// reason other than the file being unreadable (which wipes it instead).
    pub fn open(path: &Path, key: Key) -> Result<(Self, Opened), StoreError> {
        let created = create_private(path)?;
        let (conn, opened) = if let Some(conn) = connect(path, &key)? {
            let opened = if created {
                Opened::Created
            } else {
                Opened::Existing
            };
            (conn, opened)
        } else {
            erase_files(path)?;
            create_private(path)?;
            let conn = connect(path, &key)?.ok_or(CryptoError::Unreadable)?;
            (conn, Opened::Wiped)
        };
        let store = Self {
            path: path.to_owned(),
            conn,
            key,
            rows: HashMap::new(),
            next_order: 1,
        };
        Ok((store, opened))
    }

    /// Reads every stored Item into a new History.
    ///
    /// Items run through the detectors again, unless the user demoted them
    /// with an Override: one a newer detector now flags comes back
    /// Suspected, starts its Expiry from `now` and leaves the disk on the
    /// next [`Store::sync`]. Rows that fail to decrypt are erased and counted.
    ///
    /// # Errors
    ///
    /// [`StoreError::Database`] if SQLite fails.
    pub fn load(
        &mut self,
        settings: Settings,
        now: Instant,
        wall_now: SystemTime,
    ) -> Result<Loaded, StoreError> {
        let rows = self
            .conn
            .prepare("SELECT id, meta, content FROM items")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<(i64, Vec<u8>, Vec<u8>)>, _>>()?;

        let mut restored = Vec::new();
        let mut unreadable = Vec::new();
        for (id, meta, content) in rows {
            match self.open_row(ItemId(id.cast_unsigned()), &meta, &content) {
                Ok(row) => restored.push(row),
                Err(_) => unreadable.push(id),
            }
        }
        for id in &unreadable {
            self.conn.execute("DELETE FROM items WHERE id = ?1", [id])?;
        }

        restored.sort_by_key(|(_, meta, _)| Reverse(meta.order));
        let mut pins: Vec<(u64, ItemId)> = Vec::new();
        let mut items = Vec::with_capacity(restored.len());
        self.rows.clear();
        for (id, meta, body) in restored {
            let captured_at = instant_at(meta.captured_ms, now, wall_now);
            self.rows.insert(
                id,
                Row {
                    order: meta.order,
                    captured_at,
                    pin_slot: meta.pin_slot,
                    overridden: meta.overridden,
                },
            );
            self.next_order = self.next_order.max(meta.order + 1);
            if meta.pin_slot > 0 {
                pins.push((meta.pin_slot, id));
            }
            let capture = Capture {
                content: body.content,
                kind: meta.kind,
                plain_fallback: body.plain_fallback,
                password_manager_hint: false,
            };
            let (sensitivity, reason) = if meta.overridden {
                (Sensitivity::Plain, Reason::Override)
            } else {
                classify(&capture, settings.detectors)
            };
            items.push(Item {
                id,
                content: capture.content,
                kind: capture.kind,
                plain_fallback: capture.plain_fallback,
                sensitivity,
                reason,
                captured_at,
                expires_at: None,
            });
        }
        pins.sort_unstable();
        let pins = pins.into_iter().map(|(_, id)| id).collect();
        Ok(Loaded {
            history: History::restore(settings, items, pins, now),
            unreadable: unreadable.len(),
        })
    }

    /// Makes the disk match `history`: writes new and changed Plain Items and
    /// erases the rest, in one transaction. A Plain Item whose content is
    /// already stored only has its small `meta` record rewritten.
    ///
    /// # Errors
    ///
    /// [`StoreError`] if sealing or SQLite fails; the disk is then unchanged.
    pub fn sync(
        &mut self,
        history: &History,
        now: Instant,
        wall_now: SystemTime,
    ) -> Result<(), StoreError> {
        let slots: HashMap<ItemId, u64> = history
            .pins()
            .zip(1..)
            .map(|(item, slot)| (item.id, slot))
            .collect();
        let mut rows = HashMap::new();
        let mut next_order = self.next_order;
        let tx = self.conn.transaction()?;
        for item in history.items().iter().rev().filter(|item| persists(item)) {
            let old = self.rows.get(&item.id);
            let order = match old {
                Some(old) if old.captured_at == item.captured_at => old.order,
                _ => {
                    let order = next_order;
                    next_order += 1;
                    order
                }
            };
            let row = Row {
                order,
                captured_at: item.captured_at,
                pin_slot: slots.get(&item.id).copied().unwrap_or(0),
                overridden: item.reason == Reason::Override,
            };
            if old != Some(&row) {
                let meta = Meta {
                    order,
                    captured_ms: wall_ms(item.captured_at, now, wall_now),
                    pin_slot: row.pin_slot,
                    overridden: row.overridden,
                    kind: item.kind.clone(),
                };
                let meta = crypto::seal(&self.key, &aad(b"meta", item.id), &meta.encode())?;
                if old.is_some() {
                    tx.execute(
                        "UPDATE items SET meta = ?2 WHERE id = ?1",
                        params![item.id.0.cast_signed(), meta],
                    )?;
                } else {
                    let body = Body::encode(&item.content, item.plain_fallback.as_ref());
                    let content = crypto::seal(&self.key, &aad(b"content", item.id), &body)?;
                    tx.execute(
                        "INSERT INTO items (id, meta, content) VALUES (?1, ?2, ?3)",
                        params![item.id.0.cast_signed(), meta, content],
                    )?;
                }
            }
            rows.insert(item.id, row);
        }
        for id in self.rows.keys().filter(|id| !rows.contains_key(id)) {
            tx.execute("DELETE FROM items WHERE id = ?1", [id.0.cast_signed()])?;
        }
        tx.commit()?;
        self.rows = rows;
        self.next_order = next_order;
        Ok(())
    }

    /// Deletes the store at `path` and its SQLite journals without opening
    /// it, for when the History must be in memory only and a store is left
    /// on disk. A missing file is fine.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] if a file exists but can't be removed.
    pub fn erase(path: &Path) -> Result<(), StoreError> {
        erase_files(path)?;
        Ok(())
    }

    /// Closes the store and deletes its files, for when the user switches to
    /// keeping the History in memory only.
    ///
    /// # Errors
    ///
    /// [`StoreError`] if a file can't be removed.
    pub fn destroy(self) -> Result<(), StoreError> {
        let Self { path, conn, .. } = self;
        conn.close().map_err(|(_, err)| err)?;
        erase_files(&path)?;
        Ok(())
    }

    fn open_row(
        &self,
        id: ItemId,
        meta: &[u8],
        content: &[u8],
    ) -> Result<(ItemId, Meta, Body), CryptoError> {
        let meta = Meta::decode(&crypto::open(&self.key, &aad(b"meta", id), meta)?)?;
        let body = Body::decode(&crypto::open(&self.key, &aad(b"content", id), content)?)?;
        Ok((id, meta, body))
    }
}

/// Whether an Item belongs on disk.
fn persists(item: &Item) -> bool {
    let size = item.content.len() + item.plain_fallback.as_ref().map_or(0, crate::Content::len);
    item.sensitivity.may_persist() && size <= MAX_PERSISTED_BYTES
}

/// Associated data binding a sealed value to its column and Item.
fn aad(column: &[u8], id: ItemId) -> Vec<u8> {
    let mut aad = b"copyveil/v1/".to_vec();
    aad.extend_from_slice(column);
    aad.push(b'/');
    aad.extend_from_slice(&id.0.to_le_bytes());
    aad
}

/// Opens SQLite and checks the key. `None` when the file can't be read with
/// this key: it is corrupt, not a database, or sealed with another key.
fn connect(path: &Path, key: &Key) -> Result<Option<Connection>, StoreError> {
    let conn = Connection::open(path)?;
    match prepare(&conn, key) {
        Ok(true) => Ok(Some(conn)),
        Ok(false) => Ok(None),
        Err(StoreError::Database(err))
            if matches!(
                err.sqlite_error_code(),
                Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt)
            ) =>
        {
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

/// Sets up the schema and the key check. `false` when the key check fails.
fn prepare(conn: &Connection, key: &Key) -> Result<bool, StoreError> {
    conn.pragma_update(None, "secure_delete", true)?;
    conn.execute_batch(SCHEMA)?;
    let sealed: Option<Vec<u8>> = conn
        .query_row("SELECT sealed FROM key_check WHERE id = 1", [], |row| {
            row.get(0)
        })
        .optional()?;
    if let Some(sealed) = sealed {
        return Ok(crypto::open(key, CHECK_AAD, &sealed)
            .is_ok_and(|plaintext| plaintext.as_slice() == CHECK_PLAINTEXT));
    }
    let sealed = crypto::seal(key, CHECK_AAD, CHECK_PLAINTEXT)?;
    conn.execute(
        "INSERT INTO key_check (id, sealed) VALUES (1, ?1)",
        [sealed],
    )?;
    Ok(true)
}

/// Creates `path` with mode 0600. `false` if it already existed.
fn create_private(path: &Path) -> io::Result<bool> {
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(err) => Err(err),
    }
}

/// Removes the database and any journal SQLite left beside it.
fn erase_files(path: &Path) -> io::Result<()> {
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        match fs::remove_file(&file) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
    }
    Ok(())
}

/// The wall-clock time, in milliseconds since the Unix epoch, of `instant`.
fn wall_ms(instant: Instant, now: Instant, wall_now: SystemTime) -> u64 {
    wall_now
        .checked_sub(now.saturating_duration_since(instant))
        .and_then(|wall| wall.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// The `Instant` of a wall-clock time from [`wall_ms`]. Times in the future
/// or before the process's clock can express become `now`.
fn instant_at(ms: u64, now: Instant, wall_now: SystemTime) -> Instant {
    let age = UNIX_EPOCH
        .checked_add(Duration::from_millis(ms))
        .and_then(|wall| wall_now.duration_since(wall).ok())
        .unwrap_or(Duration::ZERO);
    now.checked_sub(age).unwrap_or(now)
}

#[cfg(test)]
mod tests;
