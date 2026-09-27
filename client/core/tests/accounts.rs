//! Accounts (PLAN §12.2 "one database per account", §12.7, D6, D14, D22, D25): isolation,
//! switching, sign-out with the outbox check, disabled / deletion-pending accounts, login
//! error mapping and session expiry.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

mod common;

use common::{Harness, SERVER, USER_A, USER_B};
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::net::NetError;
use strata_core::store::StorePaths;
use strata_core::store::notes;
use strata_core::sync::engine::Trigger;
use strata_core::view::hub::Recorder;
use strata_core::view::model::{
    AccountSummary, KnownAccountItem, SessionKind, SessionState, SignInRequest, SignOutOutcome,
};
use ulid::Ulid;

fn db_path(h: &Harness, user: &str) -> std::path::PathBuf {
    StorePaths::new(h.dir.path()).account(Ulid::from_string(user).expect("ulid"))
}

fn summary(user_id: &str, username: &str, timezone: &str) -> AccountSummary {
    AccountSummary {
        user_id: user_id.to_owned(),
        username: username.to_owned(),
        display_name: username.to_owned(),
        role: "member".to_owned(),
        is_admin: false,
        server_url: SERVER.to_owned(),
        timezone: timezone.to_owned(),
        ui_language: "en".to_owned(),
    }
}

fn known(user_id: &str, username: &str) -> KnownAccountItem {
    KnownAccountItem {
        user_id: user_id.to_owned(),
        username: username.to_owned(),
        display_name: username.to_owned(),
        server_url: SERVER.to_owned(),
    }
}

fn note_paths(s: &strata_core::session::Session) -> Vec<String> {
    s.read(|c, _| notes::live_paths(c))
        .expect("paths")
        .into_iter()
        .map(|(_, p)| p)
        .collect()
}

#[tokio::test]
async fn each_account_has_its_own_file_and_switching_never_mixes_data() {
    let h = Harness::new();
    let a = h.sign_in_a().await;
    a.create_note("notes/a-only.md", "# A only\n", false)
        .expect("note a");
    drop(a);

    let b = h.sign_in("mona", "pw-b").await;
    assert_eq!(note_paths(&b), Vec::<String>::new());
    b.create_note("notes/b-only.md", "# B only\n", false)
        .expect("note b");
    assert_eq!(note_paths(&b), ["notes/b-only.md"]);
    assert_eq!(
        h.core.state().expect("state"),
        SessionState {
            account: Some(summary(USER_B, "mona", "Europe/London")),
            ..SessionState::of(SessionKind::Active)
        }
    );
    drop(b);

    assert!(db_path(&h, USER_A).exists());
    assert!(db_path(&h, USER_B).exists());
    assert_ne!(db_path(&h, USER_A), db_path(&h, USER_B));

    // Switch back to A without logging in again: A's data only.
    let state = h.core.switch_account(USER_A).expect("switch");
    assert_eq!(
        state,
        SessionState {
            account: Some(summary(USER_A, "shawket", "Africa/Cairo")),
            ..SessionState::of(SessionKind::Active)
        }
    );
    let a = h.core.session().expect("session");
    assert_eq!(a.user_id().to_string(), USER_A);
    assert_eq!(note_paths(&a), ["notes/a-only.md"]);

    // Unknown accounts cannot be switched to.
    assert_eq!(
        h.core.switch_account("01K5DSSE00000000000000CCCC"),
        Err(CoreError::NotFound {
            what: "account".into()
        })
    );
}

#[tokio::test]
async fn the_active_account_survives_a_restart() {
    let mut h = Harness::new();
    let a = h.sign_in_a().await;
    a.capture("before restart").expect("capture");
    drop(a);
    h.restart();
    let a = h.core.session().expect("still signed in");
    assert_eq!(a.user_id().to_string(), USER_A);
    assert_eq!(a.unsynced().expect("unsynced"), 1);
    assert_eq!(h.core.state().expect("state").kind, SessionKind::Active);
}

#[tokio::test]
async fn sign_out_checks_the_outbox_then_deletes_the_database_and_tokens() {
    let h = Harness::new();
    let a = h.sign_in_a().await;
    a.capture("one").expect("capture");
    a.capture("two").expect("capture");
    drop(a);

    // Unsynced ops: ask first, nothing deleted.
    assert_eq!(
        h.core.sign_out(false).await.expect("sign out"),
        SignOutOutcome {
            signed_out: false,
            unsynced_ops: 2,
        }
    );
    assert!(db_path(&h, USER_A).exists());
    assert_eq!(h.core.state().expect("state").kind, SessionKind::Active);
    assert!(
        !h.accounts
            .calls
            .lock()
            .unwrap()
            .contains(&"logout".to_owned())
    );

    // Confirmed: the session is revoked, the file and the registry entry are gone.
    assert_eq!(
        h.core.sign_out(true).await.expect("sign out"),
        SignOutOutcome {
            signed_out: true,
            unsynced_ops: 0,
        }
    );
    assert!(!db_path(&h, USER_A).exists());
    assert!(
        h.accounts
            .calls
            .lock()
            .unwrap()
            .contains(&"logout".to_owned())
    );
    assert_eq!(
        h.core.state().expect("state"),
        SessionState {
            server_url: Some(SERVER.to_owned()),
            device_name: "Shawket's laptop".to_owned(),
            ..SessionState::of(SessionKind::SignedOut)
        }
    );
    assert_eq!(h.core.session().err(), Some(CoreError::NotSignedIn));
}

#[tokio::test]
async fn sign_out_after_a_sync_needs_no_confirmation_and_keeps_other_accounts() {
    let h = Harness::new();
    let b = h.sign_in("mona", "pw-b").await;
    b.capture("mona's").expect("capture");
    drop(b);
    let a = h.sign_in_a().await;
    a.capture("synced").expect("capture");
    a.sync(Trigger::Manual).await.expect("sync");
    assert_eq!(a.unsynced().expect("unsynced"), 0);
    drop(a);

    assert_eq!(
        h.core.sign_out(false).await.expect("sign out"),
        SignOutOutcome {
            signed_out: true,
            unsynced_ops: 0,
        }
    );
    assert!(!db_path(&h, USER_A).exists());
    assert!(db_path(&h, USER_B).exists());
    assert_eq!(
        h.core.state().expect("state"),
        SessionState {
            known_accounts: vec![known(USER_B, "mona")],
            server_url: Some(SERVER.to_owned()),
            device_name: "Shawket's laptop".to_owned(),
            ..SessionState::of(SessionKind::SignedOut)
        }
    );
    // B's unsynced capture is still there.
    h.core.switch_account(USER_B).expect("switch");
    assert_eq!(
        h.core.session().expect("b").unsynced().expect("unsynced"),
        1
    );
}

#[tokio::test]
async fn a_disabled_account_warns_then_wipes_on_acknowledgement() {
    let h = Harness::new();
    let a = h.sign_in_a().await;
    a.capture("never synced").expect("capture");
    let states = Recorder::new();
    h.core.watch_state(Box::new(states.clone())).expect("watch");

    h.accounts
        .update_user("shawket", |me| "disabled".clone_into(&mut me.status));
    h.core.refresh_account().await.expect("refresh");

    let disabled = SessionState {
        account: Some(summary(USER_A, "shawket", "Africa/Cairo")),
        unsynced_ops: 1,
        ..SessionState::of(SessionKind::Disabled)
    };
    assert_eq!(
        states.take(),
        vec![
            SessionState {
                account: Some(summary(USER_A, "shawket", "Africa/Cairo")),
                ..SessionState::of(SessionKind::Active)
            },
            disabled.clone(),
        ]
    );
    // Writes are refused while the warning shows; the data is kept until acknowledged.
    assert_eq!(a.capture("more"), Err(CoreError::AccountDisabled));
    assert!(db_path(&h, USER_A).exists());
    drop(a);

    let signed_out = SessionState {
        server_url: Some(SERVER.to_owned()),
        device_name: "Shawket's laptop".to_owned(),
        ..SessionState::of(SessionKind::SignedOut)
    };
    assert_eq!(
        h.core.acknowledge_disabled().expect("ack"),
        signed_out.clone()
    );
    assert!(!db_path(&h, USER_A).exists());
    assert_eq!(states.take(), vec![signed_out]);
}

#[tokio::test]
async fn a_sync_refused_with_account_disabled_switches_to_the_warning() {
    let h = Harness::new();
    let a = h.sign_in_a().await;
    a.account_failure(&NetError::AccountDisabled)
        .expect("failure");
    assert_eq!(h.core.state().expect("state").kind, SessionKind::Disabled);
    // Acknowledging is only possible in that state.
    h.core.switch_account(USER_A).expect("same account");
    drop(a);
    let b = h.sign_in("mona", "pw-b").await;
    drop(b);
    assert_eq!(
        h.core.acknowledge_disabled(),
        Err(CoreError::InvalidInput {
            field: "state".into(),
            reason: "not_disabled".into()
        })
    );
}

#[tokio::test]
async fn deletion_pending_is_export_only() {
    let h = Harness::new();
    h.accounts.update_user("shawket", |me| {
        "deletion_pending".clone_into(&mut me.status);
        me.deletion_at = Some(
            chrono::DateTime::parse_from_rfc3339("2026-10-27T10:00:00Z")
                .expect("ts")
                .into(),
        );
    });
    let a = h.sign_in_a().await;
    assert_eq!(
        h.core.state().expect("state"),
        SessionState {
            account: Some(summary(USER_A, "shawket", "Africa/Cairo")),
            deletion_at: Some(
                chrono::DateTime::parse_from_rfc3339("2026-10-27T10:00:00Z")
                    .expect("ts")
                    .into()
            ),
            days_remaining: Some(30),
            ..SessionState::of(SessionKind::DeletionPending)
        }
    );
    assert_eq!(a.capture("x"), Err(CoreError::AccountDeletionPending));
    assert_eq!(
        a.create_note("notes/x.md", "x", false),
        Err(CoreError::AccountDeletionPending)
    );
    assert_eq!(a.unsynced().expect("unsynced"), 0);
}

#[tokio::test]
async fn login_errors_map_to_typed_errors_and_leave_no_session() {
    let h = Harness::new();
    h.accounts
        .login_errors
        .lock()
        .unwrap()
        .insert("waiting".into(), NetError::AccountPending);
    h.accounts
        .login_errors
        .lock()
        .unwrap()
        .insert("refused".into(), NetError::AccountRejected);
    let req = |username: &str, password: &str| SignInRequest {
        server_url: format!("{SERVER}/"),
        username: username.to_owned(),
        password: password.to_owned(),
        device_name: "Phone".to_owned(),
    };
    assert_eq!(
        h.core.sign_in(req("waiting", "x")).await,
        Err(CoreError::AccountPending)
    );
    assert_eq!(
        h.core.sign_in(req("refused", "x")).await,
        Err(CoreError::AccountRejected)
    );
    assert_eq!(
        h.core.sign_in(req("shawket", "wrong")).await,
        Err(CoreError::InvalidCredentials)
    );
    assert_eq!(h.core.session().err(), Some(CoreError::NotSignedIn));
    assert!(!db_path(&h, USER_A).exists());
    assert_eq!(
        CoreError::AccountPending.message_key(),
        "error.account_pending"
    );

    // The trailing slash is dropped; the device name is remembered for the next login.
    let state = h.core.sign_in(req("shawket", "pw-a")).await.expect("ok");
    assert_eq!(state.account.expect("account").server_url, SERVER);
}

#[tokio::test]
async fn a_refused_refresh_ends_the_session_but_keeps_the_data() {
    let h = Harness::new();
    let a = h.sign_in_a().await;
    a.capture("kept").expect("capture");
    *h.accounts.refresh_error.lock().unwrap() = Some(NetError::Unauthorized);

    let refreshed = a.tokens().refresh().await.expect("refresh call");
    assert_eq!(refreshed, None);
    drop(a);

    assert_eq!(
        h.core.state().expect("state"),
        SessionState {
            known_accounts: vec![known(USER_A, "shawket")],
            server_url: Some(SERVER.to_owned()),
            device_name: "Shawket's laptop".to_owned(),
            ..SessionState::of(SessionKind::SignedOut)
        }
    );
    assert!(db_path(&h, USER_A).exists());

    // Signing in again reuses the database: the unsynced capture is still queued.
    let a = h.sign_in_a().await;
    assert_eq!(a.unsynced().expect("unsynced"), 1);
}

#[tokio::test]
async fn refresh_rotates_the_tokens_stored_in_the_account_database() {
    let mut h = Harness::new();
    let a = h.sign_in_a().await;
    let t = a.tokens();
    assert_eq!(
        t.access_token().await.expect("token"),
        Some("access-1".into())
    );
    assert_eq!(t.refresh().await.expect("x"), Some("access-2".into()));
    assert_eq!(t.refresh().await.expect("y"), Some("access-3".into()));
    let refreshes: Vec<String> = h
        .accounts
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.starts_with("refresh:"))
        .cloned()
        .collect();
    // Each refresh spends the previous (single-use) refresh token.
    assert_eq!(refreshes, ["refresh:refresh-1", "refresh:refresh-2"]);
    drop((t, a));

    // The rotated tokens live in the account's database (D14): they survive a restart.
    h.restart();
    let t = h.core.session().expect("session").tokens();
    assert_eq!(
        t.access_token().await.expect("token"),
        Some("access-3".into())
    );
}
