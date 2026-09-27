//! The trait implementations over the generated `strata-client`.

use std::fmt;

use futures::future::BoxFuture;
use strata_client::{Client, operations, types};

use crate::net::{AccountApi, EventsApi, MeInfo, NetError, SessionTokens, SyncApi, Tokens, classify};
use crate::sync::model::{BootstrapPage, ChangesPage, OpOutcome, SyncOp};
use crate::view::model::{AdminUserItem, Platform};

fn client(server_url: &str, tokens: Option<Tokens>) -> Result<Client, NetError> {
    let mut b = Client::builder(server_url).timeout(std::time::Duration::from_secs(30));
    if let Some(t) = tokens {
        b = b.tokens(t);
    }
    b.build().map_err(|e| classify(&e))
}

fn platform(p: Platform) -> types::DevicePlatform {
    match p {
        Platform::Android => types::DevicePlatform::Android,
        Platform::Ios => types::DevicePlatform::Ios,
        Platform::Macos => types::DevicePlatform::Macos,
        Platform::Windows => types::DevicePlatform::Windows,
        Platform::Linux => types::DevicePlatform::Linux,
    }
}

fn session(s: types::AuthSession) -> SessionTokens {
    SessionTokens {
        user_id: s.user_id.to_string(),
        device_id: s.device_id.to_string(),
        session_id: s.session_id.to_string(),
        access_token: s.access_token,
        access_expires_at: s.access_token_expires_at,
        refresh_token: s.refresh_token,
        refresh_expires_at: s.refresh_token_expires_at,
        export_only: s.export_only,
        password_change_required: s.password_change_required,
    }
}

/// [`AccountApi`] over the generated client.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientAccountApi {}

impl AccountApi for ClientAccountApi {
    fn signup(
        &self,
        server_url: String,
        username: String,
        password: String,
        display_name: String,
    ) -> BoxFuture<'_, Result<String, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, None)?;
            let body = types::SignupRequest {
                display_name,
                password,
                username,
            };
            let r = operations::signup(&c, &body)
                .await
                .map_err(|e| classify(&e))?;
            Ok(r.username)
        })
    }

    fn login(
        &self,
        server_url: String,
        username: String,
        password: String,
        device_name: String,
        p: Platform,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, None)?;
            let body = types::LoginRequest {
                device_name,
                password,
                platform: platform(p),
                username,
            };
            operations::login(&c, &body)
                .await
                .map(session)
                .map_err(|e| classify(&e))
        })
    }

    fn refresh(
        &self,
        server_url: String,
        refresh_token: String,
    ) -> BoxFuture<'_, Result<SessionTokens, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, None)?;
            operations::refresh(&c, &types::RefreshRequest { refresh_token })
                .await
                .map(session)
                .map_err(|e| classify(&e))
        })
    }

    fn logout(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            operations::logout(&c).await.map_err(|e| classify(&e))
        })
    }

    fn me(&self, server_url: String, tokens: Tokens) -> BoxFuture<'_, Result<MeInfo, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let m = operations::get_me(&c).await.map_err(|e| classify(&e))?;
            Ok(MeInfo {
                id: m.id.to_string(),
                username: m.username,
                display_name: m.display_name,
                role: m.role.to_string(),
                status: m.status.to_string(),
                timezone: m.timezone,
                ui_language: m.ui_language.to_string(),
                deletion_at: m.deletion_at,
                password_change_required: m.password_change_required,
            })
        })
    }

    fn set_device_reminders(
        &self,
        server_url: String,
        tokens: Tokens,
        device_id: String,
        enabled: bool,
    ) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let id = ulid::Ulid::from_string(&device_id)
                .map_err(|e| NetError::Protocol(format!("device id: {e}")))?;
            let body = types::UpdateDevice {
                name: None,
                reminders_enabled: Some(enabled),
            };
            operations::update_device(&c, id, &body)
                .await
                .map(|_| ())
                .map_err(|e| classify(&e))
        })
    }

    fn admin_users(
        &self,
        server_url: String,
        tokens: Tokens,
    ) -> BoxFuture<'_, Result<Vec<AdminUserItem>, NetError>> {
        Box::pin(async move {
            let c = client(&server_url, Some(tokens))?;
            let users = operations::admin_list_users(&c, None)
                .await
                .map_err(|e| classify(&e))?;
            Ok(users
                .into_iter()
                .map(|u| AdminUserItem {
                    id: u.id.to_string(),
                    username: u.username,
                    display_name: u.display_name,
                    role: u.role.to_string(),
                    status: u.status.to_string(),
                    created: u.created,
                    deletion_at: u.deletion_at,
                    export_downloaded_at: u.export_downloaded_at,
                })
                .collect())
        })
    }
}

/// [`SyncApi`] over the generated client.
///
/// **Awaiting server endpoints:** `GET /sync/bootstrap`, `GET /sync/changes` and
/// `POST /sync/push` are not in `api/openapi.json` yet, so there are no generated operations
/// to call. Every method returns [`NetError::NotAvailable`]; once the endpoints land, each
/// becomes a call to its generated operation plus a conversion into the [`crate::sync::model`]
/// (or `sync-model`) types.
#[derive(Clone)]
pub struct ClientSyncApi {
    client: Client,
}

impl fmt::Debug for ClientSyncApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientSyncApi")
            .field("base_url", &self.client.base_url())
            .finish()
    }
}

impl ClientSyncApi {
    /// A sync API for `server_url` authenticated by `tokens`.
    pub fn new(server_url: &str, tokens: Tokens) -> Result<Self, NetError> {
        Ok(Self {
            client: client(server_url, Some(tokens))?,
        })
    }
}

fn not_available<T>(endpoint: &str) -> Result<T, NetError> {
    Err(NetError::NotAvailable {
        endpoint: endpoint.to_owned(),
    })
}

impl SyncApi for ClientSyncApi {
    fn bootstrap(&self, _cursor: Option<String>) -> BoxFuture<'_, Result<BootstrapPage, NetError>> {
        Box::pin(async { not_available("sync_bootstrap") })
    }

    fn changes(
        &self,
        _since: u64,
        _epoch: u64,
        _limit: u32,
    ) -> BoxFuture<'_, Result<ChangesPage, NetError>> {
        Box::pin(async { not_available("sync_changes") })
    }

    fn push(&self, _ops: Vec<SyncOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>> {
        Box::pin(async { not_available("sync_push") })
    }
}

/// A transport that could not be built (e.g. an invalid server URL): every call fails with
/// the build error.
#[derive(Debug, Clone)]
pub struct BrokenSyncApi(pub NetError);

impl SyncApi for BrokenSyncApi {
    fn bootstrap(&self, _cursor: Option<String>) -> BoxFuture<'_, Result<BootstrapPage, NetError>> {
        Box::pin(async { Err(self.0.clone()) })
    }

    fn changes(&self, _: u64, _: u64, _: u32) -> BoxFuture<'_, Result<ChangesPage, NetError>> {
        Box::pin(async { Err(self.0.clone()) })
    }

    fn push(&self, _ops: Vec<SyncOp>) -> BoxFuture<'_, Result<Vec<OpOutcome>, NetError>> {
        Box::pin(async { Err(self.0.clone()) })
    }
}

/// [`EventsApi`] over the generated client's streaming module.
///
/// **Awaiting server endpoint:** `/events` is not in the contract yet (the generated
/// `streams` module is empty). Returns [`NetError::NotAvailable`]; the session then relies on
/// the timer and lifecycle triggers.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientEventsApi {}

impl EventsApi for ClientEventsApi {
    fn next_event(&self) -> BoxFuture<'_, Result<(), NetError>> {
        Box::pin(async { not_available("events") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sync_endpoints_report_not_available() {
        let api = ClientSyncApi::new(
            "https://strata.example",
            std::sync::Arc::new(strata_client::StaticToken("t".into())),
        )
        .expect("client");
        assert_eq!(
            api.bootstrap(None).await,
            Err(NetError::NotAvailable {
                endpoint: "sync_bootstrap".into()
            })
        );
        assert_eq!(
            api.changes(1, 1, 10).await,
            Err(NetError::NotAvailable {
                endpoint: "sync_changes".into()
            })
        );
        assert_eq!(
            api.push(Vec::new()).await,
            Err(NetError::NotAvailable {
                endpoint: "sync_push".into()
            })
        );
        assert_eq!(
            ClientEventsApi {}.next_event().await,
            Err(NetError::NotAvailable {
                endpoint: "events".into()
            })
        );
    }

    #[tokio::test]
    async fn unreachable_server_is_offline() {
        // Port 9 (discard) on localhost is closed in the test environment: connection refused.
        let r = ClientAccountApi {}
            .login(
                "http://127.0.0.1:9".into(),
                "u".into(),
                "p".into(),
                "d".into(),
                Platform::Linux,
            )
            .await;
        assert!(matches!(r, Err(NetError::Offline(_))), "{r:?}");
    }
}
