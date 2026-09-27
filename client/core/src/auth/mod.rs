//! Auth (PLAN §12.1 `auth/`, §8, D6, D14): tokens stored in the account's database, the
//! `TokenProvider` the generated client uses (refresh once on `401`, rotation of the single-use
//! refresh token), and the mapping of server account states to the app's session states.

use std::fmt;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::clock::Clock;
use crate::net::{AccountApi, NetError, SessionTokens};
use crate::store::tokens::StoredTokens;

/// Where the token provider reads and writes tokens (the account database, D14).
pub trait TokenStore: Send + Sync + fmt::Debug {
    /// The stored tokens.
    fn load(&self) -> Option<StoredTokens>;
    /// Replaces the stored tokens.
    fn save(&self, tokens: &StoredTokens);
    /// The refresh token was refused: the session is over (sign in again).
    fn session_ended(&self);
}

/// Converts login/refresh results for storage.
pub fn stored(t: &SessionTokens, now: &str) -> StoredTokens {
    StoredTokens {
        device_id: t.device_id.clone(),
        session_id: t.session_id.clone(),
        access_token: t.access_token.clone(),
        access_expires_at: t.access_expires_at.to_rfc3339(),
        refresh_token: t.refresh_token.clone(),
        refresh_expires_at: t.refresh_expires_at.to_rfc3339(),
        export_only: t.export_only,
        updated_at: now.to_owned(),
    }
}

/// The core's [`strata_client::TokenProvider`]: the current access token from the store; on
/// `401`, one refresh with the stored refresh token (rotating it). Concurrent refreshes are
/// serialised, and a caller that waited behind a successful refresh reuses its result instead
/// of spending the (single-use) refresh token again.
pub struct CoreTokenProvider {
    store: Arc<dyn TokenStore>,
    api: Arc<dyn AccountApi>,
    server_url: String,
    clock: Arc<dyn Clock>,
    refresh_lock: tokio::sync::Mutex<()>,
}

impl fmt::Debug for CoreTokenProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreTokenProvider")
            .field("server_url", &self.server_url)
            .finish_non_exhaustive()
    }
}

impl CoreTokenProvider {
    /// A provider for `server_url`.
    pub fn new(
        store: Arc<dyn TokenStore>,
        api: Arc<dyn AccountApi>,
        server_url: String,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            api,
            server_url,
            clock,
            refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    /// Refreshes now. `Ok(None)`: the session cannot be refreshed (ended).
    pub async fn refresh_now(&self, failed_access: Option<String>) -> Result<Option<String>, NetError> {
        let _guard = self.refresh_lock.lock().await;
        let Some(current) = self.store.load() else {
            return Ok(None);
        };
        if failed_access.is_some() && failed_access.as_deref() != Some(current.access_token.as_str())
        {
            // Someone refreshed while we waited: use their token.
            return Ok(Some(current.access_token));
        }
        match self
            .api
            .refresh(self.server_url.clone(), current.refresh_token.clone())
            .await
        {
            Ok(t) => {
                let now = self.clock.now().to_rfc3339();
                self.store.save(&stored(&t, &now));
                Ok(Some(t.access_token))
            }
            Err(NetError::Unauthorized | NetError::InvalidCredentials) => {
                self.store.session_ended();
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
}

impl strata_client::TokenProvider for CoreTokenProvider {
    fn access_token(&self) -> BoxFuture<'_, Result<Option<String>, strata_client::Error>> {
        Box::pin(async move { Ok(self.store.load().map(|t| t.access_token)) })
    }

    fn refresh(&self) -> BoxFuture<'_, Result<Option<String>, strata_client::Error>> {
        Box::pin(async move {
            let failed = self.store.load().map(|t| t.access_token);
            self.refresh_now(failed)
                .await
                .map_err(|e| strata_client::Error::Auth(e.to_string()))
        })
    }
}

/// How the app should treat an account, from the server's status and session flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountMode {
    /// Normal use.
    Active,
    /// Change the password first.
    PasswordChangeRequired,
    /// Disabled: warn, then wipe local data (§12.7).
    Disabled,
    /// Scheduled for deletion: export-only, local data read-only (§12.7, D25).
    DeletionPending,
}

/// Maps a server status (`users.status`) and session flags to the app's mode.
pub fn account_mode(status: &str, password_change_required: bool) -> AccountMode {
    match status {
        "disabled" | "rejected" => AccountMode::Disabled,
        "deletion_pending" => AccountMode::DeletionPending,
        _ if password_change_required => AccountMode::PasswordChangeRequired,
        _ => AccountMode::Active,
    }
}

#[cfg(test)]
mod tests;
