//! The multi-account runtime (§12.7): one signed-in account at a time, one database per
//! account, sign-in / sign-up / sign-out / switch, and the session-state stream.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use ulid::Ulid;

use super::Session;
use crate::auth::stored;
use crate::clock::{Clock, SystemClock};
use crate::error::{CoreError, CoreResult};
use crate::ids::{IdGenerator, UlidGenerator};
use crate::net::client::{ClientAccountApi, ClientSyncApi};
use crate::net::{AccountApi, NetError, SyncApi, Tokens};
use crate::store::registry::{self, KnownAccount, Registry};
use crate::store::{StorePaths, account, tokens};
use crate::sync::merge::{NoteMerger, PendingMerge};
use crate::view::model::{
    CoreConfig, KnownAccountItem, Platform, SessionState, SignInRequest, SignOutOutcome,
    SignUpOutcome, SignUpRequest,
};
use crate::view::{Topics, ViewSink};

/// Builds the sync transport of a session from its server URL and token provider.
pub type SyncApiFactory = Arc<dyn Fn(&str, Tokens) -> Arc<dyn SyncApi> + Send + Sync>;

/// Everything injectable: time, IDs, storage location, network.
pub struct CoreEnv {
    /// Where databases live.
    pub paths: StorePaths,
    /// This install's platform.
    pub platform: Platform,
    /// Time.
    pub clock: Arc<dyn Clock>,
    /// IDs.
    pub ids: Arc<dyn IdGenerator>,
    /// Account endpoints.
    pub account_api: Arc<dyn AccountApi>,
    /// Sync endpoints per session.
    pub sync_api: SyncApiFactory,
    /// Conflict merge previews.
    pub merger: Arc<dyn NoteMerger>,
    /// Default device name for the login form.
    pub default_device_name: String,
    /// Default server URL for the login form.
    pub default_server_url: Option<String>,
}

impl std::fmt::Debug for CoreEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreEnv")
            .field("paths", &self.paths)
            .field("platform", &self.platform)
            .finish_non_exhaustive()
    }
}

impl CoreEnv {
    /// The production environment for `config`.
    pub fn production(config: &CoreConfig) -> Self {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        Self {
            paths: StorePaths::new(std::path::Path::new(&config.app_data_dir)),
            platform: config.platform,
            ids: Arc::new(UlidGenerator::new(clock.clone())),
            clock,
            account_api: Arc::new(ClientAccountApi),
            sync_api: Arc::new(|url: &str, tokens: Tokens| -> Arc<dyn SyncApi> {
                match ClientSyncApi::new(url, tokens) {
                    Ok(api) => Arc::new(api),
                    Err(e) => Arc::new(crate::net::client::BrokenSyncApi(e)),
                }
            }),
            merger: Arc::new(PendingMerge),
            default_device_name: config.default_device_name.clone(),
            default_server_url: config.default_server_url.clone(),
        }
    }
}

/// The core: active session, registry, session-state stream.
pub struct Core {
    env: Arc<CoreEnv>,
    registry: Mutex<Registry>,
    active: Mutex<Option<Arc<Session>>>,
    state_sinks: Mutex<Vec<Box<dyn ViewSink<SessionState>>>>,
    last_state: Mutex<Option<SessionState>>,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core").field("env", &self.env).finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn login_error(e: NetError) -> CoreError {
    CoreError::from(e)
}

impl Core {
    /// Opens the core: the registry, and the active account's session if there is one.
    pub fn open(env: CoreEnv) -> CoreResult<Self> {
        let env = Arc::new(env);
        let registry = Registry::open(&env.paths)?;
        let active = match registry.active()? {
            Some(a) => {
                let id = crate::store::parse_ulid(&a.user_id)?;
                Some(Session::open(env.clone(), id)?)
            }
            None => None,
        };
        Ok(Self {
            env,
            registry: Mutex::new(registry),
            active: Mutex::new(active),
            state_sinks: Mutex::new(Vec::new()),
            last_state: Mutex::new(None),
        })
    }

    /// The environment.
    pub fn env(&self) -> &Arc<CoreEnv> {
        &self.env
    }

    /// The active session.
    pub fn session(&self) -> CoreResult<Arc<Session>> {
        lock(&self.active).clone().ok_or(CoreError::NotSignedIn)
    }

    fn signed_out(&self) -> CoreResult<SessionState> {
        let reg = lock(&self.registry);
        Ok(SessionState::SignedOut {
            known_accounts: reg
                .accounts()?
                .into_iter()
                .map(|a| KnownAccountItem {
                    user_id: a.user_id,
                    username: a.username,
                    display_name: a.display_name,
                    server_url: a.server_url,
                })
                .collect(),
            server_url: reg
                .device_value(registry::LAST_SERVER_URL)?
                .or_else(|| self.env.default_server_url.clone()),
            device_name: reg
                .device_value(registry::DEVICE_NAME)?
                .unwrap_or_else(|| self.env.default_device_name.clone()),
        })
    }

    /// The current session state.
    pub fn state(&self) -> CoreResult<SessionState> {
        let Some(session) = lock(&self.active).clone() else {
            return self.signed_out();
        };
        match session.session_state()? {
            SessionState::SignedOut { .. } => {
                // The session ended (refresh refused): keep the data, show the login screen.
                session.close();
                *lock(&self.active) = None;
                lock(&self.registry).deactivate_all()?;
                self.signed_out()
            }
            s => Ok(s),
        }
    }

    /// Streams the session state (emits now, then on every change).
    pub fn watch_state(&self, sink: Box<dyn ViewSink<SessionState>>) -> CoreResult<()> {
        let state = self.state()?;
        if sink.emit(state.clone()) {
            lock(&self.state_sinks).push(sink);
        }
        *lock(&self.last_state) = Some(state);
        Ok(())
    }

    /// Re-emits the session state if it changed.
    pub fn publish_state(&self) -> CoreResult<SessionState> {
        let state = self.state()?;
        let mut last = lock(&self.last_state);
        if last.as_ref() != Some(&state) {
            lock(&self.state_sinks).retain(|s| s.emit(state.clone()));
            *last = Some(state.clone());
        }
        Ok(state)
    }

    /// Registers an account (D22): it waits for approval, no session.
    pub async fn sign_up(&self, req: SignUpRequest) -> CoreResult<SignUpOutcome> {
        let username = self
            .env
            .account_api
            .signup(
                req.server_url.clone(),
                req.username,
                req.password,
                req.display_name,
            )
            .await
            .map_err(login_error)?;
        lock(&self.registry).set_device_value(registry::LAST_SERVER_URL, &req.server_url)?;
        Ok(SignUpOutcome { username })
    }

    /// Signs in as a device; opens (or reuses) the account's own database.
    pub async fn sign_in(&self, req: SignInRequest) -> CoreResult<SessionState> {
        let server_url = req.server_url.trim_end_matches('/').to_owned();
        let t = self
            .env
            .account_api
            .login(
                server_url.clone(),
                req.username.clone(),
                req.password,
                req.device_name.clone(),
                self.env.platform,
            )
            .await
            .map_err(login_error)?;
        let user_id = crate::store::parse_ulid(&t.user_id)?;
        let now = self.env.clock.now().to_rfc3339();
        self.deactivate()?;
        let session = Session::open(self.env.clone(), user_id)?;
        session.write(|c, _| {
            tokens::put(c, &stored(&t, &now))?;
            let existing = account::get(c)?;
            account::put(
                c,
                &account::AccountRow {
                    user_id: t.user_id.clone(),
                    username: req.username.clone(),
                    display_name: existing
                        .as_ref()
                        .map_or_else(|| req.username.clone(), |a| a.display_name.clone()),
                    role: existing
                        .as_ref()
                        .map_or_else(|| "member".to_owned(), |a| a.role.clone()),
                    status: if t.export_only {
                        "deletion_pending".to_owned()
                    } else {
                        "active".to_owned()
                    },
                    server_url: server_url.clone(),
                    timezone: existing
                        .as_ref()
                        .map_or_else(|| "UTC".to_owned(), |a| a.timezone.clone()),
                    ui_language: existing
                        .as_ref()
                        .map_or_else(|| "en".to_owned(), |a| a.ui_language.clone()),
                    deletion_at: existing.as_ref().and_then(|a| a.deletion_at.clone()),
                    password_change_required: t.password_change_required,
                    disabled_warned_at: None,
                },
            )?;
            Ok(((), Topics::ACCOUNT))
        })?;
        // The session's token provider and transport were built before the account row
        // existed; reopen so they carry the server URL.
        session.close();
        drop(session);
        let session = Session::open(self.env.clone(), user_id)?;
        {
            let mut reg = lock(&self.registry);
            reg.activate(&KnownAccount {
                user_id: t.user_id.clone(),
                username: req.username.clone(),
                display_name: req.username.clone(),
                server_url: server_url.clone(),
                last_active_at: now.clone(),
                active: true,
            })?;
            reg.set_device_value(registry::LAST_SERVER_URL, &server_url)?;
            reg.set_device_value(registry::DEVICE_NAME, &req.device_name)?;
        }
        *lock(&self.active) = Some(session.clone());
        // Best effort: the profile (role, timezone, deletion date). Offline keeps defaults.
        let _ = self.refresh_account().await;
        self.publish_state()
    }

    /// Fetches `GET /me` and updates the account row.
    pub async fn refresh_account(&self) -> CoreResult<()> {
        let session = self.session()?;
        let url = session.server_url()?;
        let me = match self.env.account_api.me(url, session.tokens()).await {
            Ok(me) => me,
            Err(e) => {
                session.account_failure(&e)?;
                self.publish_state()?;
                return Err(e.into());
            }
        };
        session.write(|c, _| {
            account::update(c, |a| {
                a.username.clone_from(&me.username);
                a.display_name.clone_from(&me.display_name);
                a.role.clone_from(&me.role);
                a.status.clone_from(&me.status);
                a.timezone.clone_from(&me.timezone);
                a.ui_language.clone_from(&me.ui_language);
                a.deletion_at = me.deletion_at.map(|d| d.to_rfc3339());
                a.password_change_required = me.password_change_required;
            })?;
            Ok(((), Topics::ACCOUNT | Topics::TASKS))
        })?;
        lock(&self.registry).activate(&KnownAccount {
            user_id: me.id.clone(),
            username: me.username.clone(),
            display_name: me.display_name.clone(),
            server_url: session.server_url()?,
            last_active_at: self.env.clock.now().to_rfc3339(),
            active: true,
        })?;
        self.publish_state()?;
        Ok(())
    }

    fn deactivate(&self) -> CoreResult<()> {
        if let Some(s) = lock(&self.active).take() {
            s.close();
        }
        lock(&self.registry).deactivate_all()
    }

    /// Switches to another account that has a database on this device (its session tokens
    /// are reused). Data never mixes: each account has its own file.
    pub fn switch_account(&self, user_id: &str) -> CoreResult<SessionState> {
        let known = lock(&self.registry)
            .accounts()?
            .into_iter()
            .find(|a| a.user_id == user_id)
            .ok_or_else(|| CoreError::not_found("account"))?;
        let id: Ulid = crate::store::parse_ulid(user_id)?;
        self.deactivate()?;
        let session = Session::open(self.env.clone(), id)?;
        lock(&self.registry).activate(&KnownAccount {
            last_active_at: self.env.clock.now().to_rfc3339(),
            active: true,
            ..known
        })?;
        *lock(&self.active) = Some(session);
        self.publish_state()
    }

    /// Signs out (§12.7): with unsynced ops and no `force`, asks for confirmation; otherwise
    /// cancels reminders, revokes the session (best effort) and deletes the account's database
    /// and tokens.
    pub async fn sign_out(&self, force: bool) -> CoreResult<SignOutOutcome> {
        let session = self.session()?;
        let unsynced = session.unsynced()?;
        if unsynced > 0 && !force {
            return Ok(SignOutOutcome::NeedsConfirmation {
                unsynced_ops: unsynced,
            });
        }
        session.cancel_all_notifications()?;
        let url = session.server_url()?;
        // Revoking the server session is best effort: offline sign-out still removes local data.
        let _ = self.env.account_api.logout(url, session.tokens()).await;
        self.wipe(session)?;
        Ok(SignOutOutcome::SignedOut)
    }

    fn wipe(&self, session: Arc<Session>) -> CoreResult<()> {
        let user_id = session.user_id().to_string();
        *lock(&self.active) = None;
        lock(&self.registry).remove(&user_id)?;
        session.destroy()?;
        self.publish_state()?;
        Ok(())
    }

    /// The user saw the "account disabled" warning: local data is wiped (§12.7).
    pub fn acknowledge_disabled(&self) -> CoreResult<SessionState> {
        let session = self.session()?;
        if !matches!(session.session_state()?, SessionState::Disabled { .. }) {
            return Err(CoreError::invalid("state", "not_disabled"));
        }
        session.cancel_all_notifications()?;
        self.wipe(session)?;
        self.state()
    }
}
