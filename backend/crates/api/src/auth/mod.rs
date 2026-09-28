//! Authentication and account management (PLAN §5.2, §7.5 "Auth & devices" and "Account &
//! admin", §8, D6 = b, D22 = b, D25 = a). The design is written up in `docs/ARCHITECTURE.md`,
//! section "Auth".
//!
//! - [`tokens`]: Ed25519-signed access tokens, refresh-token generation and hashing.
//! - [`password`]: Argon2id hashing with configured parameters; temporary passwords.
//! - [`username`]: NFKC case folding and UTS #39 confusable skeletons.
//! - [`revocation`]: the in-memory revocation set with periodic reload.
//! - [`rate_limit`]: in-process sliding-window limiters.
//! - [`middleware`]: bearer-token authentication, export-only and password-change
//!   restrictions, the [`Authenticated`] extractor carrying the caller's `UserScope`.
//! - [`provision`]: the [`VaultProvisioner`] seam (create/remove per-user vaults).
//! - [`service`]: sessions, sign-up, login, refresh, account purge.
//! - [`export`]: the streamed vault zip of `GET /me/export`.

pub mod error;
pub mod export;
pub mod middleware;
pub mod password;
pub mod provision;
pub mod rate_limit;
pub mod revocation;
pub mod service;
pub mod tokens;
pub mod username;

use std::fmt;
use std::sync::Arc;

use chrono::Duration;
use sqlx::PgPool;
use strata_common::{Clock, Config, IdGenerator};
use strata_index::{AccountsDb, AppDb, ScopeIssuer};

pub use error::AccountError;
pub use middleware::{AuthContext, Authenticated};
pub use provision::{DataRoot, GitVaultProvisioner, VaultProvisioner};
pub use revocation::RevocationSet;
pub use service::{purge_account, purge_due_accounts};
pub use tokens::{SigningKeys, TokenService};

use middleware::Authenticator;
use password::PasswordHasher;
use rate_limit::AuthLimiters;

/// Settings the account endpoints read at request time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSettings {
    /// Absolute lifetime of a device session.
    pub session_ttl: Duration,
    /// Grace period before a scheduled deletion is purged (D25).
    pub deletion_grace: Duration,
    /// Cap on accounts waiting for approval.
    pub max_pending_signups: u32,
    /// Minimum password length (characters).
    pub min_password_length: usize,
    /// Take the client IP from forwarding headers (behind nginx only).
    pub trust_forwarded_for: bool,
    /// Timezone reported for users who have not chosen one.
    pub default_timezone: String,
}

impl AuthSettings {
    /// The settings from the server configuration.
    pub fn from_config(config: &Config) -> Self {
        Self {
            session_ttl: Duration::days(i64::from(config.auth.session_ttl_days)),
            deletion_grace: config.deletion_grace_period(),
            max_pending_signups: config.accounts.max_pending_signups,
            min_password_length: usize::try_from(config.auth.min_password_length)
                .unwrap_or(usize::MAX),
            trust_forwarded_for: config.auth.trust_forwarded_for,
            default_timezone: config.default_timezone.clone(),
        }
    }
}

/// Everything the auth middleware and the account endpoints need, registered as
/// `web::Data<AuthState>` on the app.
pub struct AuthState {
    pub(crate) accounts: AccountsDb,
    /// `strata_accounts` pool, for account-table statements `AccountsDb` does not cover.
    pub(crate) accounts_pool: PgPool,
    pub(crate) app_db: AppDb,
    pub(crate) authenticator: Authenticator,
    pub(crate) tokens: TokenService,
    pub(crate) revocations: Arc<RevocationSet>,
    pub(crate) limiters: AuthLimiters,
    pub(crate) passwords: PasswordHasher,
    pub(crate) vaults: Arc<dyn VaultProvisioner>,
    pub(crate) clock: Arc<dyn Clock>,
    pub(crate) ids: Arc<dyn IdGenerator>,
    pub(crate) settings: AuthSettings,
}

impl fmt::Debug for AuthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthState")
            .field("tokens", &self.tokens)
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

/// The parts [`AuthState::new`] is assembled from (the composition root owns them).
#[derive(Debug)]
pub struct AuthDeps {
    /// Pool connected as `strata_accounts`.
    pub accounts_pool: PgPool,
    /// The `strata_app` handle.
    pub app_db: AppDb,
    /// Its scope issuer: handed to the middleware only.
    pub issuer: ScopeIssuer,
    /// The access-token signing key.
    pub keys: SigningKeys,
    /// Vault directory provisioning.
    pub vaults: Arc<dyn VaultProvisioner>,
    /// Time source.
    pub clock: Arc<dyn Clock>,
    /// ID source.
    pub ids: Arc<dyn IdGenerator>,
}

impl AuthState {
    /// Assembles the state from `deps` and the server configuration.
    pub fn new(deps: AuthDeps, config: &Config) -> Result<Self, AccountError> {
        let access_ttl = Duration::seconds(i64::from(config.auth.access_token_ttl_secs));
        Ok(Self {
            accounts: AccountsDb::new(deps.accounts_pool.clone()),
            accounts_pool: deps.accounts_pool,
            app_db: deps.app_db,
            authenticator: Authenticator::new(deps.issuer),
            tokens: TokenService::new(
                deps.keys,
                config.auth.issuer.clone(),
                config.auth.audience.clone(),
                access_ttl,
                deps.clock.clone(),
            ),
            revocations: Arc::new(RevocationSet::new(access_ttl, deps.clock.clone())),
            limiters: AuthLimiters::new(&config.auth.rate_limits, &deps.clock),
            passwords: PasswordHasher::new(&config.auth.argon2)?,
            vaults: deps.vaults,
            clock: deps.clock,
            ids: deps.ids,
            settings: AuthSettings::from_config(config),
        })
    }

    /// The revocation set (shared with the periodic reload task).
    pub fn revocations(&self) -> &Arc<RevocationSet> {
        &self.revocations
    }

    /// Reloads the revocation set from the database.
    pub async fn reload_revocations(&self) -> strata_index::Result<()> {
        self.revocations
            .reload(&self.accounts, &self.accounts_pool)
            .await
    }

    /// The access-token service.
    pub fn tokens(&self) -> &TokenService {
        &self.tokens
    }

    /// Counts one `POST /capture` of `user`; `429 rate_limited` past
    /// `auth.rate_limits.capture_per_user` (PLAN §8, §15).
    pub fn check_capture_limit(&self, user: strata_common::UserId) -> Result<(), AccountError> {
        self.limiters
            .capture_user
            .check(&user.to_string())
            .map_err(|l| AccountError::RateLimited {
                retry_after_secs: l.retry_after_secs,
                reason: "too many captures; try again later",
            })
    }

    /// Counts one `POST /ask` of `user`; `429 rate_limited` past
    /// `auth.rate_limits.ask_per_user` (PLAN §8, §15).
    pub fn check_ask_limit(&self, user: strata_common::UserId) -> Result<(), AccountError> {
        self.limiters
            .ask_user
            .check(&user.to_string())
            .map_err(|l| AccountError::RateLimited {
                retry_after_secs: l.retry_after_secs,
                reason: "too many questions; try again later",
            })
    }

    /// Creates an active account with its vault (as `POST /admin/users` does; `actor` is the
    /// creating admin, `None` for the system).
    pub async fn create_account(
        &self,
        input: &service::NewAccount<'_>,
        actor: Option<strata_common::UserId>,
    ) -> Result<strata_index::accounts::User, AccountError> {
        service::create_active_account(
            &self.accounts,
            &self.passwords,
            self.vaults.as_ref(),
            self.ids.as_ref(),
            self.settings.min_password_length,
            input,
            actor,
            self.clock.now(),
        )
        .await
    }
}
