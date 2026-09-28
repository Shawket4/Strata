//! The fixed server address through the core (owner decisions "App ID, server address, HTTPS
//! first" and "No server address in the UI"): the build's `STRATA_SERVER_URL` must be
//! `https://` (plain `http://` only for this device, in debug builds), a blank or refused one
//! stops the core from opening, and every account's sign-up, sign-in, profile and sync use it
//! without any address coming from the UI.

#![allow(clippy::expect_used, clippy::unwrap_used)]

mod common;

use std::sync::{Arc, Mutex};

use common::USER_A;
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::clock::FakeClock;
use strata_core::ids::SeqIds;
use strata_core::net::{SyncApi, Tokens};
use strata_core::session::{Core, CoreEnv};
use strata_core::sync::engine::Trigger;
use strata_core::testing::{FakeAccountApi, FakeServer};
use strata_core::view::model::{
    CoreConfig, CoreFailure, Platform, SessionKind, SignInRequest, SignUpRequest,
};
use tempfile::TempDir;

const DUCKDNS: &str = "https://strata-ai.duckdns.org";

fn config(dir: &TempDir, url: &str, release_build: bool) -> CoreConfig {
    CoreConfig {
        app_data_dir: dir.path().to_string_lossy().into_owned(),
        platform: Platform::Linux,
        default_device_name: "laptop".to_owned(),
        server_url: url.to_owned(),
        release_build,
    }
}

/// The production environment's server address for `url`, or its failure as Dart sees it.
fn production(url: &str, release_build: bool) -> Result<String, CoreFailure> {
    let dir = TempDir::new().expect("dir");
    CoreEnv::production(&config(&dir, url, release_build))
        .map(|env| env.server_url)
        .map_err(CoreFailure::from)
}

fn misconfigured(reason: &str) -> Result<String, CoreFailure> {
    Err(CoreFailure {
        code: "misconfigured_build".to_owned(),
        message_key: "error.misconfigured_build".to_owned(),
        field: Some("server_url".to_owned()),
        reason: Some(reason.to_owned()),
        count: None,
        status: None,
    })
}

#[test]
fn https_is_the_server_of_release_and_debug_builds() {
    for release in [true, false] {
        assert_eq!(production(DUCKDNS, release), Ok(DUCKDNS.to_owned()));
        assert_eq!(
            production(" https://strata-ai.duckdns.org/ ", release),
            Ok(DUCKDNS.to_owned())
        );
    }
}

#[test]
fn a_blank_server_address_stops_every_build() {
    for release in [true, false] {
        for url in ["", "   ", "/"] {
            assert_eq!(
                production(url, release),
                misconfigured("missing"),
                "{url:?}"
            );
        }
    }
}

#[test]
fn plain_http_to_another_host_stops_every_build() {
    for release in [true, false] {
        for url in [
            "http://strata-ai.duckdns.org",
            "http://187.124.33.153:8080",
            "http://127.0.0.2:8080",
        ] {
            assert_eq!(
                production(url, release),
                misconfigured("insecure_http"),
                "{url}"
            );
        }
    }
}

#[test]
fn loopback_http_is_for_debug_builds_only() {
    for url in [
        "http://127.0.0.1:8080",
        "http://localhost:8080",
        "http://[::1]:8080",
    ] {
        assert_eq!(production(url, false), Ok(url.to_owned()), "{url}");
        assert_eq!(
            production(&format!("{url}/"), false),
            Ok(url.to_owned()),
            "{url}"
        );
        assert_eq!(
            production(url, true),
            misconfigured("insecure_http"),
            "{url}"
        );
    }
}

#[test]
fn an_address_without_https_stops_every_build() {
    for release in [true, false] {
        for url in [
            "strata-ai.duckdns.org",
            "ftp://strata-ai.duckdns.org",
            "https://",
        ] {
            assert_eq!(
                production(url, release),
                misconfigured("not_https"),
                "{url}"
            );
        }
    }
}

#[test]
fn a_misconfigured_build_has_a_stable_error() {
    let dir = TempDir::new().expect("dir");
    let err = CoreEnv::production(&config(&dir, "", true)).expect_err("refused");
    assert_eq!(
        err,
        CoreError::MisconfiguredBuild {
            reason: "missing".to_owned(),
        }
    );
    assert_eq!(err.message_key(), "error.misconfigured_build");
    assert_eq!(
        err.to_string(),
        "misconfigured build: server address missing"
    );
}

/// A core whose build server is [`DUCKDNS`]; the sync transport records the URL it was
/// built for.
fn core_on_duckdns(
    dir: &TempDir,
    accounts: &Arc<FakeAccountApi>,
) -> (Core, Arc<Mutex<Vec<String>>>) {
    let server = FakeServer::new();
    let mut env = common::env(
        dir.path(),
        &server,
        accounts,
        &FakeClock::at(common::NOW),
        &Arc::new(SeqIds::new(common::ID_BASE_MS)),
        Platform::Android,
    );
    DUCKDNS.clone_into(&mut env.server_url);
    let sync_urls = Arc::new(Mutex::new(Vec::new()));
    let seen = sync_urls.clone();
    env.sync_api = Arc::new(move |url: &str, _tokens: Tokens| -> Arc<dyn SyncApi> {
        seen.lock().unwrap().push(url.to_owned());
        Arc::new(server.clone())
    });
    (Core::open(env).expect("core opens"), sync_urls)
}

#[tokio::test]
async fn sign_up_sign_in_profile_and_sync_use_the_build_server() {
    let dir = TempDir::new().expect("dir");
    let accounts = Arc::new(FakeAccountApi::default());
    accounts.add_user("shawket", "pw-a", USER_A, "Africa/Cairo");
    let (core, sync_urls) = core_on_duckdns(&dir, &accounts);

    core.sign_up(SignUpRequest {
        username: "nour".to_owned(),
        password: "a long enough password".to_owned(),
        display_name: "Nour".to_owned(),
    })
    .await
    .expect("sign up");
    assert_eq!(
        core.state().expect("state").kind,
        SessionKind::PendingApproval
    );
    core.dismiss_pending().expect("dismiss");

    let state = core
        .sign_in(SignInRequest {
            username: "shawket".to_owned(),
            password: "pw-a".to_owned(),
            device_name: "Shawket's phone".to_owned(),
        })
        .await
        .expect("sign in");
    assert_eq!(state.kind, SessionKind::Active);
    assert_eq!(state.account.expect("account").server_url, DUCKDNS);

    let session = core.session().expect("session");
    assert_eq!(session.server_url(), DUCKDNS);
    session
        .capture("synced to the build server")
        .expect("capture");
    session.sync(Trigger::Manual).await.expect("sync");
    assert_eq!(session.unsynced().expect("unsynced"), 0);

    assert_eq!(
        accounts.calls.lock().unwrap().clone(),
        ["signup:nour", "login:shawket", "me"]
    );
    assert_eq!(accounts.servers.lock().unwrap().clone(), [DUCKDNS; 3]);
    // One transport per opened session, each for the build's server.
    let urls = sync_urls.lock().unwrap().clone();
    assert!(!urls.is_empty());
    assert_eq!(urls, vec![DUCKDNS.to_owned(); urls.len()]);
}

#[tokio::test]
async fn a_restarted_core_keeps_using_the_build_server() {
    let dir = TempDir::new().expect("dir");
    let accounts = Arc::new(FakeAccountApi::default());
    accounts.add_user("shawket", "pw-a", USER_A, "Africa/Cairo");
    {
        let (core, _) = core_on_duckdns(&dir, &accounts);
        core.sign_in(SignInRequest {
            username: "shawket".to_owned(),
            password: "pw-a".to_owned(),
            device_name: "Shawket's phone".to_owned(),
        })
        .await
        .expect("sign in");
    }
    let (core, sync_urls) = core_on_duckdns(&dir, &accounts);
    let state = core.state().expect("state");
    assert_eq!(state.kind, SessionKind::Active);
    assert_eq!(state.account.expect("account").server_url, DUCKDNS);
    assert_eq!(sync_urls.lock().unwrap().clone(), [DUCKDNS]);
    core.sign_out(true).await.expect("sign out");
    assert_eq!(
        accounts.servers.lock().unwrap().clone(),
        [DUCKDNS; 3],
        "login, me, logout"
    );
}
