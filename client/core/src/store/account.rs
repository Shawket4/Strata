//! `account`: the signed-in user's profile and server status (one row).

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CoreResult;

/// The account row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRow {
    /// User ID.
    pub user_id: String,
    /// Username.
    pub username: String,
    /// Display name.
    pub display_name: String,
    /// `admin` | `member`.
    pub role: String,
    /// Account status (`active`, `disabled`, `deletion_pending`, …).
    pub status: String,
    /// Server base URL.
    pub server_url: String,
    /// IANA timezone.
    pub timezone: String,
    /// UI language.
    pub ui_language: String,
    /// When a scheduled deletion purges the account (RFC 3339).
    pub deletion_at: Option<String>,
    /// The password must be changed first.
    pub password_change_required: bool,
    /// When the user was warned that the disabled account's data will be wiped.
    pub disabled_warned_at: Option<String>,
}

/// Reads the account.
pub fn get(conn: &Connection) -> CoreResult<Option<AccountRow>> {
    Ok(conn
        .query_row(
            "SELECT user_id, username, display_name, role, status, server_url, timezone,
                    ui_language, deletion_at, password_change_required, disabled_warned_at
             FROM account WHERE singleton = 1",
            [],
            |r| {
                Ok(AccountRow {
                    user_id: r.get(0)?,
                    username: r.get(1)?,
                    display_name: r.get(2)?,
                    role: r.get(3)?,
                    status: r.get(4)?,
                    server_url: r.get(5)?,
                    timezone: r.get(6)?,
                    ui_language: r.get(7)?,
                    deletion_at: r.get(8)?,
                    password_change_required: r.get(9)?,
                    disabled_warned_at: r.get(10)?,
                })
            },
        )
        .optional()?)
}

/// Replaces the account row.
pub fn put(conn: &Connection, a: &AccountRow) -> CoreResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO account (singleton, user_id, username, display_name, role, status,
             server_url, timezone, ui_language, deletion_at, password_change_required,
             disabled_warned_at)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            a.user_id,
            a.username,
            a.display_name,
            a.role,
            a.status,
            a.server_url,
            a.timezone,
            a.ui_language,
            a.deletion_at,
            a.password_change_required,
            a.disabled_warned_at
        ],
    )?;
    Ok(())
}

/// Updates the account row with `f` (no-op without an account).
pub fn update(
    conn: &Connection,
    f: impl FnOnce(&mut AccountRow),
) -> CoreResult<Option<AccountRow>> {
    let Some(mut a) = get(conn)? else {
        return Ok(None);
    };
    f(&mut a);
    put(conn, &a)?;
    Ok(Some(a))
}
