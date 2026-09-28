//! Forward-only schema migrations, tracked in `PRAGMA user_version`.
//!
//! A migration list is append-only: shipped migrations are never edited or reordered. Each
//! migration runs in its own transaction together with its `user_version` bump, so a crash
//! leaves the database at a whole version. Opening a database newer than this build knows is
//! refused rather than guessed at.

use rusqlite::Connection;

use crate::error::{CoreError, CoreResult};

/// One migration: the version it produces and its SQL.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// `user_version` after this migration.
    pub version: u32,
    /// Short name (for errors and tests).
    pub name: &'static str,
    /// The SQL batch.
    pub sql: &'static str,
}

/// Account database migrations.
pub const ACCOUNT: &[Migration] = &[
    Migration {
        version: 1,
        name: "cache",
        sql: include_str!("migrations/account_0001_cache.sql"),
    },
    Migration {
        version: 2,
        name: "notifications",
        sql: include_str!("migrations/account_0002_notifications.sql"),
    },
    Migration {
        version: 3,
        name: "server",
        sql: include_str!("migrations/account_0003_server.sql"),
    },
    Migration {
        version: 4,
        name: "custody_note",
        sql: include_str!("migrations/account_0004_custody_note.sql"),
    },
];

/// Device registry migrations.
pub const REGISTRY: &[Migration] = &[Migration {
    version: 1,
    name: "registry",
    sql: include_str!("migrations/registry_0001.sql"),
}];

/// The schema version of a database.
pub fn version(conn: &Connection) -> CoreResult<u32> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Applies every migration newer than the database's version.
pub fn migrate(conn: &Connection, migrations: &[Migration]) -> CoreResult<u32> {
    migrate_to(conn, migrations, u32::MAX)
}

/// Applies migrations up to and including `target` (tests build fixtures at old versions).
pub fn migrate_to(conn: &Connection, migrations: &[Migration], target: u32) -> CoreResult<u32> {
    let mut current = version(conn)?;
    let latest = migrations.last().map_or(0, |m| m.version);
    if current > latest {
        return Err(CoreError::Storage(format!(
            "database schema v{current} is newer than this app (v{latest})"
        )));
    }
    let from = current;
    for m in migrations
        .iter()
        .filter(|m| m.version > from && m.version <= target)
    {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(m.sql)
            .map_err(|e| CoreError::Storage(format!("migration {} failed: {e}", m.name)))?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
        current = m.version;
    }
    Ok(current)
}
