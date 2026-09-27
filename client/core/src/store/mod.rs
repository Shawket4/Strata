//! Local SQLite storage (PLAN §12.2, §12.7).
//!
//! - One database file **per account**, keyed by user ID, under
//!   `<app data>/strata/accounts/<user id>.sqlite3` ([`AccountDb`]). Account switching opens a
//!   different file, so data never mixes; sign-out deletes the file.
//! - A device registry `<app data>/strata/registry.sqlite3` ([`registry::Registry`]) that only
//!   knows which accounts exist on the device and which is active.
//! - Forward-only migrations ([`migrations`]) tracked in `PRAGMA user_version`.
//! - [`write`] is the write path of §12.3: an intent's local change and its outbox op commit in
//!   one transaction.

pub mod account;
pub mod conflicts;
pub mod index;
pub mod migrations;
pub mod notes;
pub mod notifications;
pub mod outbox;
pub mod queries;
pub mod registry;
pub mod settings;
pub mod sync_state;
pub mod tokens;
pub mod write;

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use ulid::Ulid;

use crate::error::{CoreError, CoreResult};

/// Directory under the app-data directory that holds every Strata file.
pub const ROOT_DIR: &str = "strata";

/// File name of the device registry.
pub const REGISTRY_FILE: &str = "registry.sqlite3";

/// Directory of the per-account databases.
pub const ACCOUNTS_DIR: &str = "accounts";

/// Where the databases of one app install live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePaths {
    root: PathBuf,
}

impl StorePaths {
    /// Paths under `app_data_dir` (the platform's app-support directory, from Dart's
    /// `path_provider` or a test temp dir).
    pub fn new(app_data_dir: &Path) -> Self {
        Self {
            root: app_data_dir.join(ROOT_DIR),
        }
    }

    /// The registry database.
    pub fn registry(&self) -> PathBuf {
        self.root.join(REGISTRY_FILE)
    }

    /// The database of `user_id`.
    pub fn account(&self, user_id: Ulid) -> PathBuf {
        self.root
            .join(ACCOUNTS_DIR)
            .join(format!("{user_id}.sqlite3"))
    }

    /// Creates the directories.
    pub fn ensure(&self) -> CoreResult<()> {
        std::fs::create_dir_all(self.root.join(ACCOUNTS_DIR))?;
        Ok(())
    }
}

/// The database of one account.
#[derive(Debug)]
pub struct AccountDb {
    conn: Connection,
    path: PathBuf,
    user_id: Ulid,
}

impl AccountDb {
    /// Opens (creating and migrating if needed) the database of `user_id`.
    pub fn open(paths: &StorePaths, user_id: Ulid) -> CoreResult<Self> {
        paths.ensure()?;
        let path = paths.account(user_id);
        let conn = open_connection(&path)?;
        migrations::migrate(&conn, migrations::ACCOUNT)?;
        Ok(Self {
            conn,
            path,
            user_id,
        })
    }

    /// Opens an in-memory database (unit tests).
    pub fn open_in_memory(user_id: Ulid) -> CoreResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrations::migrate(&conn, migrations::ACCOUNT)?;
        Ok(Self {
            conn,
            path: PathBuf::new(),
            user_id,
        })
    }

    /// The account this database belongs to.
    pub fn user_id(&self) -> Ulid {
        self.user_id
    }

    /// The file path (empty for in-memory databases).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The connection (read queries).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// The connection for transactions.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Closes the connection and deletes the file and its WAL/SHM companions (sign-out,
    /// disabled-account wipe).
    pub fn delete(self) -> CoreResult<()> {
        let path = self.path.clone();
        drop(self.conn);
        delete_db_files(&path)
    }
}

/// Deletes a database file and its `-wal`/`-shm` companions (missing files are fine).
pub fn delete_db_files(path: &Path) -> CoreResult<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        match std::fs::remove_file(PathBuf::from(p)) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub(crate) fn open_connection(path: &Path) -> CoreResult<Connection> {
    let conn = Connection::open(path)?;
    // WAL keeps readers and the writer apart; NORMAL is durable across app crashes (only an OS
    // crash can lose the last transactions, which the outbox then re-sends).
    conn.execute_batch(
        "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
    )?;
    Ok(conn)
}

/// Encodes a value as MessagePack (named maps, like the wire, L21).
pub(crate) fn to_msgpack<T: serde::Serialize>(value: &T) -> CoreResult<Vec<u8>> {
    Ok(rmp_serde::to_vec_named(value)?)
}

/// Decodes MessagePack.
pub(crate) fn from_msgpack<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> CoreResult<T> {
    Ok(rmp_serde::from_slice(bytes)?)
}

/// Parses a stored ULID column.
pub(crate) fn parse_ulid(s: &str) -> CoreResult<Ulid> {
    Ulid::from_string(s).map_err(|e| CoreError::Storage(format!("bad ulid {s:?}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_keyed_by_user() {
        let p = StorePaths::new(Path::new("/data"));
        let u = Ulid::from_parts(1, 1);
        assert_eq!(p.registry(), PathBuf::from("/data/strata/registry.sqlite3"));
        assert_eq!(
            p.account(u),
            PathBuf::from("/data/strata/accounts/00000000010000000000000001.sqlite3")
        );
    }
}
