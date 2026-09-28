//! The multi-account runtime (§12.7): one signed-in account at a time, one database per
//! account, sign-in / sign-up / sign-out / switch, and the session-state stream.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use ulid::Ulid;

use super::Session;
use crate::auth::stored;
use crate::clock::{Clock, SystemClock};
use crate::error::{CoreError, CoreResult};
use crate::ids::{IdGenerator, UlidGenerator};
use crate::net::client::{ClientAccountApi, ClientEventsApi, ClientSyncApi};
use crate::net::{AccountApi, EventsApi, NetError, SyncApi, Tokens};
use crate::store::registry::{self, KnownAccount, Registry};
use crate::store::{StorePaths, account, tokens};
use crate::view::model::{
    CoreConfig, KnownAccountItem, NotificationOp, PasswordLevel, PasswordStrength, PendingApproval,
    Platform, SessionKind, SessionState, SignInRequest, SignOutOutcome, SignUpOutcome,
    SignUpRequest,
};
use crate::view::{Topics, ViewSink};

/// The notification adapter's stream(s): kept by the core, not by a session, so the adapter
/// can subscribe while signed out (it receives nothing then) and keeps its subscription across
/// sign-in, switching and sign-out.
#[derive(Default)]
pub struct NotifyHub {
    sinks: Mutex<Vec<Box<dyn ViewSink<NotificationOp>>>>,
}

impl std::fmt::Debug for NotifyHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotifyHub")
            .field("sinks", &lock(&self.sinks).len())
            .finish()
    }
}

impl NotifyHub {
    /// Adds a stream.
    pub fn attach(&self, sink: Box<dyn ViewSink<NotificationOp>>) {
        lock(&self.sinks).push(sink);
    }

    /// Whether any stream is attached.
    pub fn is_attached(&self) -> bool {
        !lock(&self.sinks).is_empty()
    }

    /// Sends one op to every stream (closed streams are dropped).
    pub fn emit(&self, op: &NotificationOp) {
        lock(&self.sinks).retain(|s| s.emit(op.clone()));
    }
}

/// The server's default minimum password length (`auth.min_password_length`).
pub const MIN_PASSWORD_LENGTH: u32 = 10;

/// Password strength for the sign-up meter (the server enforces only the minimum length).
pub fn password_strength(password: &str) -> PasswordStrength {
    let length = u32::try_from(password.chars().count()).unwrap_or(u32::MAX);
    let classes = [
        password.chars().any(char::is_lowercase),
        password.chars().any(char::is_uppercase),
        password.chars().any(char::is_numeric),
        password.chars().any(|c| !c.is_alphanumeric()),
    ]
    .iter()
    .filter(|b| **b)
    .count();
    let level = if length < MIN_PASSWORD_LENGTH {
        PasswordLevel::TooShort
    } else if classes >= 4 || length >= 20 {
        PasswordLevel::Strong
    } else if classes >= 2 || length >= 14 {
        PasswordLevel::Fair
    } else {
        PasswordLevel::Weak
    };
    PasswordStrength {
        level,
        length,
        min_length: MIN_PASSWORD_LENGTH,
    }
}

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
    /// The `/events` stream.
    pub events_api: Arc<dyn EventsApi>,
    /// The notification adapter's streams.
    pub notifications: NotifyHub,
    /// Default device name for the login form.
    pub default_device_name: String,
    /// Default server URL for the login form.
    pub default_server_url: Option<String>,
    /// The device's IANA time zone (`None` when unknown): the time zone of an account that
    /// has not chosen one ([`Core::adopt_device_timezone`]).
    pub device_timezone: Option<String>,
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
        let clock: Arc<dyn Clock> = Arc::new(SystemClock {});
        Self {
            paths: StorePaths::new(std::path::Path::new(&config.app_data_dir)),
            platform: config.platform,
            ids: Arc::new(UlidGenerator::new(clock.clone())),
            clock,
            account_api: Arc::new(ClientAccountApi {}),
            events_api: Arc::new(ClientEventsApi {}),
            notifications: NotifyHub::default(),
            sync_api: Arc::new(|url: &str, tokens: Tokens| -> Arc<dyn SyncApi> {
                match ClientSyncApi::new(url, tokens) {
                    Ok(api) => Arc::new(api),
                    Err(e) => Arc::new(crate::net::client::BrokenSyncApi(e)),
                }
            }),
            default_device_name: config.default_device_name.clone(),
            default_server_url: crate::net::server_url::default_from_build(
                config.default_server_url.as_deref(),
            ),
            device_timezone: device_timezone(),
        }
    }
}

/// The device's IANA time zone, read from the operating system (Android, iOS, macOS, Windows
/// and Linux alike), when it names a zone the core knows.
pub fn device_timezone() -> Option<String> {
    iana_time_zone::get_timezone()
        .ok()
        .filter(|z| z.parse::<chrono_tz::Tz>().is_ok())
}

/// The core: active session, registry, session-state stream.
pub struct Core {
    env: Arc<CoreEnv>,
    registry: Mutex<Registry>,
    active: Mutex<Option<Arc<Session>>>,
    state_sinks: Mutex<Vec<Box<dyn ViewSink<SessionState>>>>,
    last_state: Mutex<Option<SessionState>>,
    /// The sign-in of an account awaiting approval, kept in memory only (it holds the
    /// password) for "Check again".
    pending_request: Mutex<Option<SignInRequest>>,
    /// Bumped whenever the active session changes (the events loop re-subscribes).
    generation: std::sync::atomic::AtomicU64,
}

impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core")
            .field("env", &self.env)
            .finish_non_exhaustive()
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
                Some(Session::open(&env, id)?)
            }
            None => None,
        };
        Ok(Self {
            env,
            registry: Mutex::new(registry),
            active: Mutex::new(active),
            state_sinks: Mutex::new(Vec::new()),
            last_state: Mutex::new(None),
            pending_request: Mutex::new(None),
            generation: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// Changes whenever the active session changes.
    pub fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn bump_generation(&self) {
        self.generation
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// The environment.
    pub fn env(&self) -> &Arc<CoreEnv> {
        &self.env
    }

    /// The active session.
    pub fn session(&self) -> CoreResult<Arc<Session>> {
        lock(&self.active).clone().ok_or(CoreError::NotSignedIn)
    }

    fn pending_approval(&self, reg: &Registry) -> CoreResult<Option<(PendingApproval, bool)>> {
        let (Some(username), Some(server_url), Some(requested)) = (
            reg.device_value(registry::PENDING_USERNAME)?,
            reg.device_value(registry::PENDING_SERVER)?,
            reg.device_value(registry::PENDING_REQUESTED_AT)?,
        ) else {
            return Ok(None);
        };
        let labels = crate::format::labels::Labels::new(
            self.env.clock.now(),
            chrono_tz::UTC,
            crate::format::labels::Lang::En,
        );
        let requested_at = crate::view::build::ts(&requested);
        let checked = reg
            .device_value(registry::PENDING_CHECKED_AT)?
            .map(|c| crate::view::build::ts(&c));
        let rejected = reg.device_value(registry::PENDING_REJECTED)?.as_deref() == Some("true");
        Ok(Some((
            PendingApproval {
                requested_label: labels.ago(requested_at),
                // Relative, so no timezone is needed before the account's is known.
                last_checked_label: checked.map(|c| format!("Last checked {}", labels.ago(c))),
                last_checked_at: checked,
                can_check: lock(&self.pending_request)
                    .as_ref()
                    .is_some_and(|r| r.username == username),
                requested_at,
                username,
                server_url,
            },
            rejected,
        )))
    }

    fn signed_out(&self) -> CoreResult<SessionState> {
        let reg = lock(&self.registry);
        let pending = self.pending_approval(&reg)?;
        let kind = match &pending {
            Some((_, true)) => SessionKind::Rejected,
            Some((_, false)) => SessionKind::PendingApproval,
            None => SessionKind::SignedOut,
        };
        Ok(SessionState {
            pending: pending.map(|(p, _)| p),
            known_accounts: reg
                .accounts()?
                .into_iter()
                .map(|a| KnownAccountItem {
                    initials: crate::format::labels::initials(&a.display_name),
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
            ..SessionState::of(kind)
        })
    }

    fn set_pending(&self, username: &str, server_url: &str, rejected: bool) -> CoreResult<()> {
        let reg = lock(&self.registry);
        let now = self.env.clock.now().to_rfc3339();
        let same = reg.device_value(registry::PENDING_USERNAME)?.as_deref() == Some(username);
        if !same {
            reg.set_device_value(registry::PENDING_USERNAME, username)?;
            reg.set_device_value(registry::PENDING_REQUESTED_AT, &now)?;
            reg.remove_device_value(registry::PENDING_CHECKED_AT)?;
        }
        reg.set_device_value(registry::PENDING_SERVER, server_url)?;
        reg.set_device_value(
            registry::PENDING_REJECTED,
            if rejected { "true" } else { "false" },
        )?;
        Ok(())
    }

    fn clear_pending(&self) -> CoreResult<()> {
        let reg = lock(&self.registry);
        for key in [
            registry::PENDING_USERNAME,
            registry::PENDING_SERVER,
            registry::PENDING_REQUESTED_AT,
            registry::PENDING_CHECKED_AT,
            registry::PENDING_REJECTED,
        ] {
            reg.remove_device_value(key)?;
        }
        *lock(&self.pending_request) = None;
        Ok(())
    }

    /// "Check again" on the waiting-for-approval screen: signs in with the request kept in
    /// memory. Still pending → the state with the new "last checked" time; rejected →
    /// `Rejected`; approved → signed in.
    pub async fn check_approval(&self) -> CoreResult<SessionState> {
        let Some(request) = lock(&self.pending_request).clone() else {
            return Err(CoreError::invalid("password", "required"));
        };
        lock(&self.registry).set_device_value(
            registry::PENDING_CHECKED_AT,
            &self.env.clock.now().to_rfc3339(),
        )?;
        match self.sign_in(request).await {
            Ok(state) => Ok(state),
            Err(CoreError::AccountPending | CoreError::AccountRejected) => self.publish_state(),
            Err(e) => Err(e),
        }
    }

    /// Leaves the waiting-for-approval / rejected screen (back to sign-in).
    pub fn dismiss_pending(&self) -> CoreResult<SessionState> {
        self.clear_pending()?;
        self.publish_state()
    }

    /// The current session state.
    pub fn state(&self) -> CoreResult<SessionState> {
        let Some(session) = lock(&self.active).clone() else {
            return self.signed_out();
        };
        match session.session_state()? {
            s if s.kind == SessionKind::SignedOut => {
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

    /// Registers an account (D22): it waits for approval, no session. A plain-`http://`
    /// server address other than this device is refused ([`crate::net::server_url`]).
    pub async fn sign_up(&self, req: SignUpRequest) -> CoreResult<SignUpOutcome> {
        let server_url = crate::net::server_url::checked(&req.server_url)?;
        let username = self
            .env
            .account_api
            .signup(
                server_url.clone(),
                req.username,
                req.password.clone(),
                req.display_name,
            )
            .await
            .map_err(login_error)?;
        lock(&self.registry).set_device_value(registry::LAST_SERVER_URL, &server_url)?;
        let device_name = lock(&self.registry)
            .device_value(registry::DEVICE_NAME)?
            .unwrap_or_else(|| self.env.default_device_name.clone());
        self.set_pending(&username, &server_url, false)?;
        *lock(&self.pending_request) = Some(SignInRequest {
            server_url,
            username: username.clone(),
            password: req.password.clone(),
            device_name,
        });
        self.publish_state()?;
        Ok(SignUpOutcome { username })
    }

    /// Signs in as a device; opens (or reuses) the account's own database. A plain-`http://`
    /// server address other than this device is refused ([`crate::net::server_url`]).
    pub async fn sign_in(&self, req: SignInRequest) -> CoreResult<SessionState> {
        let server_url = crate::net::server_url::checked(&req.server_url)?;
        let login = self
            .env
            .account_api
            .login(
                server_url.clone(),
                req.username.clone(),
                req.password.clone(),
                req.device_name.clone(),
                self.env.platform,
            )
            .await;
        let t = match login {
            Ok(t) => t,
            Err(e @ (NetError::AccountPending | NetError::AccountRejected)) => {
                // Waiting for (or refused) approval: a session state of its own (D22).
                let rejected = e == NetError::AccountRejected;
                self.set_pending(&req.username, &server_url, rejected)?;
                *lock(&self.pending_request) = (!rejected).then(|| req.clone());
                self.publish_state()?;
                return Err(login_error(e));
            }
            Err(e) => return Err(login_error(e)),
        };
        self.clear_pending()?;
        let user_id = crate::store::parse_ulid(&t.user_id)?;
        let now = self.env.clock.now().to_rfc3339();
        self.deactivate()?;
        let session = Session::open(&self.env, user_id)?;
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
                    // Until `/me` answers: the device's zone (the default of a new account).
                    timezone: existing.as_ref().map_or_else(
                        || {
                            self.env
                                .device_timezone
                                .clone()
                                .unwrap_or_else(|| "UTC".to_owned())
                        },
                        |a| a.timezone.clone(),
                    ),
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
        let session = Session::open(&self.env, user_id)?;
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
        self.bump_generation();
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
        self.bump_generation();
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
        let session = Session::open(&self.env, id)?;
        lock(&self.registry).activate(&KnownAccount {
            last_active_at: self.env.clock.now().to_rfc3339(),
            active: true,
            ..known
        })?;
        *lock(&self.active) = Some(session);
        self.bump_generation();
        self.publish_state()
    }

    /// Signs out (§12.7): with unsynced ops and no `force`, asks for confirmation; otherwise
    /// cancels reminders, revokes the session (best effort) and deletes the account's database
    /// and tokens.
    pub async fn sign_out(&self, force: bool) -> CoreResult<SignOutOutcome> {
        let session = self.session()?;
        let unsynced = session.unsynced()?;
        if unsynced > 0 && !force {
            return Ok(SignOutOutcome {
                signed_out: false,
                unsynced_ops: unsynced,
            });
        }
        session.cancel_all_notifications()?;
        let url = session.server_url()?;
        // Revoking the server session is best effort: offline sign-out still removes local data.
        let _ = self.env.account_api.logout(url, session.tokens()).await;
        self.wipe(session)?;
        Ok(SignOutOutcome {
            signed_out: true,
            unsynced_ops: 0,
        })
    }

    pub(super) fn wipe(&self, session: Arc<Session>) -> CoreResult<()> {
        let user_id = session.user_id().to_string();
        *lock(&self.active) = None;
        self.bump_generation();
        lock(&self.registry).remove(&user_id)?;
        session.destroy()?;
        self.publish_state()?;
        Ok(())
    }

    /// The user saw the "account disabled" warning: local data is wiped (§12.7).
    pub fn acknowledge_disabled(&self) -> CoreResult<SessionState> {
        let session = self.session()?;
        if session.session_state()?.kind != SessionKind::Disabled {
            return Err(CoreError::invalid("state", "not_disabled"));
        }
        session.cancel_all_notifications()?;
        self.wipe(session)?;
        self.state()
    }
}
