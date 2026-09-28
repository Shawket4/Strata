//! App-level calls: init, accounts, lifecycle.

use flutter_rust_bridge::frb;

use super::runtime::{self, core};
use super::sink::DartSink;
use super::{lift, lift_async};
use crate::frb_generated::StreamSink;
use crate::session::{Core, CoreEnv};
use crate::sync::engine::Trigger;
use crate::view::model::CoreFailure;
use crate::view::model::{
    AppLifecycle, CoreConfig, ExportSummary, PasswordStrength, SessionState, SignInRequest,
    SignOutOutcome, SignUpOutcome, SignUpRequest,
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
        let core = Core::open(CoreEnv::production(&config)?)?;
        let core = runtime::install(core);
        core.state()
    })
    .await
}

/// Streams the session state (login screen, main shell, restricted screens).
pub fn watch_session(sink: StreamSink<SessionState>) -> Result<(), CoreFailure> {
    lift(|| core()?.watch_state(Box::new(DartSink(sink))))
}

/// Registers an account; it waits for admin approval (D22).
pub async fn sign_up(request: SignUpRequest) -> Result<SignUpOutcome, CoreFailure> {
    lift_async(async { core()?.sign_up(request).await }).await
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
    lift_async(async { core()?.sign_out(force).await }).await
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
    lift(|| core()?.acknowledge_disabled())
}

/// Re-reads the profile (`GET /me`).
pub async fn refresh_account() -> Result<(), CoreFailure> {
    lift_async(async { core()?.refresh_account().await }).await
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

/// "Check again" on the waiting-for-approval screen (uses the sign-in kept in memory).
pub async fn check_approval() -> Result<SessionState, CoreFailure> {
    lift_async(async {
        let state = core()?.check_approval().await?;
        if state.kind == crate::view::model::SessionKind::Active {
            runtime::trigger(Trigger::Start);
        }
        Ok(state)
    })
    .await
}

/// Leaves the waiting-for-approval / rejected screen.
pub fn dismiss_pending() -> Result<SessionState, CoreFailure> {
    lift(|| core()?.dismiss_pending())
}

/// Password strength for the sign-up meter.
pub fn password_strength(password: String) -> PasswordStrength {
    crate::session::password_strength(&password)
}

/// Changes the password (Settings → Account, and the password-change-required screen).
pub async fn change_password(current: String, new: String) -> Result<SessionState, CoreFailure> {
    lift_async(async { core()?.change_password(&current, &new).await }).await
}

/// Sets the UI language (`en` | `ar`).
pub async fn set_ui_language(code: String) -> Result<(), CoreFailure> {
    lift_async(async { core()?.set_ui_language(&code).await }).await
}

/// Sets the time zone (IANA name, e.g. `Africa/Cairo`).
pub async fn set_timezone(iana: String) -> Result<(), CoreFailure> {
    lift_async(async { core()?.set_timezone(&iana).await }).await
}

/// Sets the display name.
pub async fn set_display_name(name: String) -> Result<(), CoreFailure> {
    lift_async(async { core()?.set_display_name(&name).await }).await
}

/// Downloads the account's export (`GET /me/export`) to a file the user chose.
pub async fn download_export(path: String) -> Result<ExportSummary, CoreFailure> {
    lift_async(async { core()?.download_export(&path).await }).await
}

/// "Delete now" (D25): refused while ops are unsynced unless `force`.
pub async fn delete_account_now(force: bool) -> Result<SessionState, CoreFailure> {
    lift_async(async { core()?.delete_account_now(force).await }).await
}

/// Writes the unsynced ops to a readable file (disabled / deletion-pending accounts); returns
/// how many.
pub fn export_unsynced(path: String) -> Result<u32, CoreFailure> {
    lift(|| core()?.session()?.export_unsynced(&path))
}

/// Pauses (or resumes) sync.
pub fn set_sync_paused(paused: bool) -> Result<(), CoreFailure> {
    lift(|| {
        core()?.session()?.set_sync_paused(paused)?;
        if !paused {
            runtime::trigger(Trigger::Manual);
        }
        Ok(())
    })
}
