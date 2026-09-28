//! The device registry: which accounts have a database on this device and which is active.
//! It holds no account content (content lives only in each account's own file).

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CoreResult;
use crate::store::{StorePaths, migrations, open_connection};

/// A known account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownAccount {
    /// User ID.
    pub user_id: String,
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// Server URL.
    pub server_url: String,
    /// Last time it was active.
    pub last_active_at: String,
    /// Currently active.
    pub active: bool,
}

/// Device-wide default: the last server URL used.
pub const LAST_SERVER_URL: &str = "last_server_url";
/// Device-wide default: the device name used at login.
pub const DEVICE_NAME: &str = "device_name";
/// A sign-up/sign-in awaiting approval: username.
pub const PENDING_USERNAME: &str = "pending_username";
/// … its server.
pub const PENDING_SERVER: &str = "pending_server";
/// … when it was requested (RFC 3339).
pub const PENDING_REQUESTED_AT: &str = "pending_requested_at";
/// … last "Check again" (RFC 3339).
pub const PENDING_CHECKED_AT: &str = "pending_checked_at";
/// … `true` once rejected.
pub const PENDING_REJECTED: &str = "pending_rejected";

/// The registry database.
#[derive(Debug)]
pub struct Registry {
    conn: Connection,
}

impl Registry {
    /// Opens (creating and migrating) the registry.
    pub fn open(paths: &StorePaths) -> CoreResult<Self> {
        paths.ensure()?;
        Self::open_at(&paths.registry())
    }

    fn open_at(path: &Path) -> CoreResult<Self> {
        let conn = open_connection(path)?;
        migrations::migrate(&conn, migrations::REGISTRY)?;
        Ok(Self { conn })
    }

    /// Every known account, most recently active first.
    pub fn accounts(&self) -> CoreResult<Vec<KnownAccount>> {
        let mut st = self.conn.prepare(
            "SELECT user_id, username, display_name, server_url, last_active_at, active
             FROM accounts ORDER BY last_active_at DESC, user_id",
        )?;
        Ok(st
            .query_map([], |r| {
                Ok(KnownAccount {
                    user_id: r.get(0)?,
                    username: r.get(1)?,
                    display_name: r.get(2)?,
                    server_url: r.get(3)?,
                    last_active_at: r.get(4)?,
                    active: r.get(5)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    /// The active account.
    pub fn active(&self) -> CoreResult<Option<KnownAccount>> {
        Ok(self.accounts()?.into_iter().find(|a| a.active))
    }

    /// Adds or updates an account and makes it the only active one.
    pub fn activate(&mut self, a: &KnownAccount) -> CoreResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE accounts SET active = 0", [])?;
        tx.execute(
            "INSERT INTO accounts (user_id, username, display_name, server_url, last_active_at, active)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)
             ON CONFLICT (user_id) DO UPDATE SET username = excluded.username,
                display_name = excluded.display_name, server_url = excluded.server_url,
                last_active_at = excluded.last_active_at, active = 1",
            params![a.user_id, a.username, a.display_name, a.server_url, a.last_active_at],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// No account is active (switching or signing out).
    pub fn deactivate_all(&self) -> CoreResult<()> {
        self.conn.execute("UPDATE accounts SET active = 0", [])?;
        Ok(())
    }

    /// Forgets an account (its database is deleted separately).
    pub fn remove(&self, user_id: &str) -> CoreResult<()> {
        self.conn
            .execute("DELETE FROM accounts WHERE user_id = ?1", [user_id])?;
        Ok(())
    }

    /// A device-wide default.
    pub fn device_value(&self, key: &str) -> CoreResult<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM device WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    /// Removes a device-wide value.
    pub fn remove_device_value(&self, key: &str) -> CoreResult<()> {
        self.conn
            .execute("DELETE FROM device WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Sets a device-wide default.
    pub fn set_device_value(&self, key: &str, value: &str) -> CoreResult<()> {
        self.conn.execute(
            "INSERT INTO device (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}
