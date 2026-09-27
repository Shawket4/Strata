//! `stratad serve`: startup checks, composition root, background tasks, HTTP server.
//!
//! Design: **one `stratad` process** per deployment (PLAN §14: one VPS, one systemd unit).
//! The revocation set and the rate limiters live in this process; the revocation set is also
//! reloaded from the database every `auth.revocation_reload_secs` (see
//! `docs/ARCHITECTURE.md`, "Auth").

use std::path::Path;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use actix_web::middleware::from_fn;
use actix_web::{App, HttpServer, web};
use strata_api::auth::{AuthDeps, AuthState, SigningKeys, purge_due_accounts};
use strata_common::{Clock, Config, IdGenerator, SystemClock, SystemIdGenerator};
use strata_index::types::UserStatus;
use strata_index::{AccountsDb, AppDb, ScopeIssuer};
use strata_vault::{VaultConfig, VaultService};
use tokio::task::JoinHandle;

use crate::checks::{StartupError, check_database_locale, check_secret_file, secret_files};

/// Reads and checks the signing key file.
pub fn load_signing_key(path: &Path) -> Result<SigningKeys, StartupError> {
    check_secret_file(path)?;
    let pem = std::fs::read_to_string(path)?;
    SigningKeys::from_pem(&pem).map_err(|e| StartupError::SigningKey {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// The shared state `serve` runs with.
#[derive(Clone)]
pub struct Prepared {
    /// Accounts, sessions, tokens.
    pub auth: web::Data<AuthState>,
    /// The vault store (also the auth layer's vault provisioner).
    pub vault: VaultService,
    /// Mints scopes for the startup reconciliation.
    pub issuer: ScopeIssuer,
    /// The accounts database (for listing users to reconcile).
    pub accounts: AccountsDb,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("vault", &self.vault)
            .finish_non_exhaustive()
    }
}

/// Runs every startup check and assembles the shared state. Refuses to continue on a secret
/// file readable by others, a missing or invalid key, a missing data root, or a database that
/// is not UTF-8 with a non-`C` character locale.
pub async fn prepare(config: &Config) -> Result<web::Data<AuthState>, StartupError> {
    Ok(prepare_all(config).await?.auth)
}

/// [`prepare`], also returning the vault store and what the startup reconciliation needs.
pub async fn prepare_all(config: &Config) -> Result<Prepared, StartupError> {
    for path in secret_files(config) {
        check_secret_file(&path)?;
    }
    let keys = load_signing_key(&config.auth.signing_key_file)?;
    if !config.data_root.is_dir() {
        return Err(StartupError::DataRoot {
            path: config.data_root.clone(),
        });
    }
    let max = config.database.max_connections;
    let app = strata_index::pool::connect(
        strata_index::pool::connect_options(&config.database.app_url)?,
        max,
    )
    .await?;
    check_database_locale(&app).await?;
    let accounts = strata_index::pool::connect(
        strata_index::pool::connect_options(&config.database.accounts_url)?,
        max,
    )
    .await?;
    let (app_db, issuer) = AppDb::new(app);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let ids: Arc<dyn IdGenerator> = Arc::new(SystemIdGenerator::new(clock.clone()));
    let vault = vault_service(config, app_db.clone(), clock.clone(), ids.clone());
    let state = AuthState::new(
        AuthDeps {
            accounts_pool: accounts.clone(),
            app_db,
            issuer: issuer.clone(),
            keys,
            vaults: Arc::new(vault.clone()),
            ids,
            clock,
        },
        config,
    )
    .map_err(|e| StartupError::Config(e.to_string()))?;
    state.reload_revocations().await?;
    Ok(Prepared {
        auth: web::Data::new(state),
        vault,
        issuer,
        accounts: AccountsDb::new(accounts),
    })
}

/// The vault store for `config` (data root, default time zone).
pub fn vault_service(
    config: &Config,
    db: AppDb,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
) -> VaultService {
    let mut vc = VaultConfig::new(&config.data_root);
    vc.default_timezone.clone_from(&config.default_timezone);
    VaultService::new(vc, db, clock, ids)
}

/// Startup reconciliation (PLAN §7.3): loads every active or deletion-pending user's vault
/// once, which removes temp files, commits out-of-band changes as `system: recovered
/// changes`, assigns missing IDs, repairs sidecars and re-derives changed notes (warnings go
/// to `GET /integrity`). Runs in the background; a request for a vault not yet reached
/// triggers the same work for that vault first.
pub fn spawn_reconciliation(prepared: &Prepared) -> JoinHandle<()> {
    let (vault, issuer, accounts) = (
        prepared.vault.clone(),
        prepared.issuer.clone(),
        prepared.accounts.clone(),
    );
    tokio::spawn(async move {
        let users = match accounts.list_users(None).await {
            Ok(users) => users,
            Err(err) => {
                tracing::error!(error = %err, "startup reconciliation: listing users failed");
                return;
            }
        };
        for user in users {
            if !matches!(user.status, UserStatus::Active | UserStatus::DeletionPending) {
                continue;
            }
            if let Err(err) = vault.ready(&issuer.issue(user.id)).await {
                tracing::error!(user = %user.id, error = %err, "startup reconciliation failed");
            }
        }
    })
}

/// Reloads the revocation set every `period`.
pub fn spawn_revocation_reload(state: web::Data<AuthState>, period: StdDuration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(period);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tick.tick().await;
        loop {
            tick.tick().await;
            if let Err(err) = state.reload_revocations().await {
                tracing::error!(error = %err, "revocation set reload failed");
            }
        }
    })
}

/// Runs the account purge job every `period` (D25).
pub fn spawn_purge(
    state: web::Data<AuthState>,
    clock: Arc<dyn Clock>,
    period: StdDuration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(period);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            match purge_due_accounts(&state, clock.now()).await {
                Ok(purged) if !purged.is_empty() => {
                    tracing::info!(count = purged.len(), "purged accounts");
                }
                Ok(_) => {}
                Err(err) => tracing::error!(error = %err, "account purge failed"),
            }
        }
    })
}

/// `stratad serve`: checks, background tasks, then the HTTP server until SIGINT/SIGTERM
/// (graceful: in-flight requests finish, then the background tasks stop).
pub async fn run(config: Config) -> Result<(), StartupError> {
    let prepared = prepare_all(&config).await?;
    let state = prepared.auth.clone();
    let vault = web::Data::new(prepared.vault.clone());
    let reconcile = spawn_reconciliation(&prepared);
    let reload = spawn_revocation_reload(
        state.clone(),
        StdDuration::from_secs(u64::from(config.auth.revocation_reload_secs)),
    );
    let purge = spawn_purge(
        state.clone(),
        Arc::new(SystemClock),
        StdDuration::from_secs(u64::from(config.accounts.purge_interval_secs)),
    );
    tracing::info!(bind = %config.bind, "stratad listening");
    let app_state = state.clone();
    let result = HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(vault.clone())
            .wrap(from_fn(crate::logging::log_request))
            .configure(strata_api::app::configure)
    })
    .shutdown_timeout(30)
    .bind(config.bind)
    .map_err(StartupError::from);
    let outcome = match result {
        Ok(server) => server.run().await.map_err(StartupError::from),
        Err(err) => Err(err),
    };
    reload.abort();
    purge.abort();
    reconcile.abort();
    tracing::info!("stratad stopped");
    outcome
}
