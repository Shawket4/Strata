//! App-level calls: init, accounts, lifecycle.

use flutter_rust_bridge::frb;

use super::runtime::{self, core};
use super::{lift, lift_async};
use super::sink::DartSink;
use crate::view::model::CoreFailure;
use crate::frb_generated::StreamSink;
use crate::session::{Core, CoreEnv};
use crate::sync::engine::Trigger;
use crate::view::model::{
    AppLifecycle, CoreConfig, SessionState, SignInRequest, SignOutOutcome, SignUpOutcome,
    SignUpRequest,
};

/// frb start-up hook (logging, panic backtraces).
#[frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_default_user_utils();
}

/// Opens the core for this install: the device registry, and the last active account's
/// database if one is signed in. Returns the session state to route on.
pub async fn init_core(config: CoreConfig) -> Result<SessionState, CoreFailure> {
    lift_async(async {
        let core = Core::open(CoreEnv::production(&config))?;
        let core = runtime::install(core);
        core.state()
    })
    .await
}

/// Streams the session state (login screen, main shell, restricted screens).
pub fn watch_session(sink: StreamSink<SessionState>) -> Result<(), CoreFailure> {
    lift(|| {
        core()?.watch_state(Box::new(DartSink(sink)))
    })
}

/// Registers an account; it waits for admin approval (D22).
pub async fn sign_up(request: SignUpRequest) -> Result<SignUpOutcome, CoreFailure> {
    lift_async(async {
        core()?.sign_up(request).await
    })
    .await
}

/// Signs in as this device.
pub async fn sign_in(request: SignInRequest) -> Result<SessionState, CoreFailure> {
    lift_async(async {
        let state = core()?.sign_in(request).await?;
        runtime::trigger(Trigger::Start);
        Ok(state)
    })
    .await
}

/// Signs out: `force = false` returns `NeedsConfirmation` while ops are unsynced.
pub async fn sign_out(force: bool) -> Result<SignOutOutcome, CoreFailure> {
    lift_async(async {
        core()?.sign_out(force).await
    })
    .await
}

/// Switches to another account with data on this device.
pub fn switch_account(user_id: String) -> Result<SessionState, CoreFailure> {
    lift(|| {
        let state = core()?.switch_account(&user_id)?;
        runtime::trigger(Trigger::Start);
        Ok(state)
    })
}

/// The user saw the "account disabled" screen: wipes the account's local data.
pub fn acknowledge_account_disabled() -> Result<SessionState, CoreFailure> {
    lift(|| {
        core()?.acknowledge_disabled()
    })
}

/// Re-reads the profile (`GET /me`).
pub async fn refresh_account() -> Result<(), CoreFailure> {
    lift_async(async {
        core()?.refresh_account().await
    })
    .await
}

/// App lifecycle (resume triggers a sync and a reminder refill, §12.4, §12.5b).
pub fn app_lifecycle(state: AppLifecycle) -> Result<(), CoreFailure> {
    lift(|| {
        core()?;
        if state == AppLifecycle::Resumed {
            runtime::trigger(Trigger::Resume);
        }
        Ok(())
    })
}

/// "Sync now".
pub fn sync_now() -> Result<(), CoreFailure> {
    lift(|| {
        core()?;
        runtime::trigger(Trigger::Manual);
        Ok(())
    })
}
