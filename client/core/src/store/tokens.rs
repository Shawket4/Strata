//! `auth_tokens`: the session tokens, stored in the account database (D14).

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CoreResult;

/// A device session's tokens.
#[derive(Clone, PartialEq, Eq)]
pub struct StoredTokens {
    /// Device ID.
    pub device_id: String,
    /// Session ID.
    pub session_id: String,
    /// Bearer access token.
    pub access_token: String,
    /// Access token expiry (RFC 3339 UTC).
    pub access_expires_at: String,
    /// Single-use refresh token.
    pub refresh_token: String,
    /// Refresh token expiry.
    pub refresh_expires_at: String,
    /// Export-only session (account scheduled for deletion).
    pub export_only: bool,
    /// Last written.
    pub updated_at: String,
}

impl std::fmt::Debug for StoredTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredTokens")
            .field("device_id", &self.device_id)
            .field("session_id", &self.session_id)
            .field("access_expires_at", &self.access_expires_at)
            .field("refresh_expires_at", &self.refresh_expires_at)
            .field("export_only", &self.export_only)
            .finish_non_exhaustive()
    }
}

/// Reads the tokens.
pub fn get(conn: &Connection) -> CoreResult<Option<StoredTokens>> {
    Ok(conn
        .query_row(
            "SELECT device_id, session_id, access_token, access_expires_at, refresh_token,
                    refresh_expires_at, export_only, updated_at
             FROM auth_tokens WHERE singleton = 1",
            [],
            |r| {
                Ok(StoredTokens {
                    device_id: r.get(0)?,
                    session_id: r.get(1)?,
                    access_token: r.get(2)?,
                    access_expires_at: r.get(3)?,
                    refresh_token: r.get(4)?,
                    refresh_expires_at: r.get(5)?,
                    export_only: r.get(6)?,
                    updated_at: r.get(7)?,
                })
            },
        )
        .optional()?)
}

/// Replaces the tokens.
pub fn put(conn: &Connection, t: &StoredTokens) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO auth_tokens (singleton, device_id, session_id, access_token,
             access_expires_at, refresh_token, refresh_expires_at, export_only, updated_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            t.device_id,
            t.session_id,
            t.access_token,
            t.access_expires_at,
            t.refresh_token,
            t.refresh_expires_at,
            t.export_only,
            t.updated_at
        ],
    )?;
    Ok(())
}

/// Deletes the tokens.
pub fn clear(conn: &Connection) -> CoreResult<()> {
    conn.execute("DELETE FROM auth_tokens", [])?;
    Ok(())
}
