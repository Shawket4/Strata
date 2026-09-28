//! The server-address rule through the core (owner decision "App ID, server address, HTTPS
//! first"): sign-in and sign-up refuse plain `http://` except to this device, before any
//! request is made; the build's default address prefills the signed-out session.

#![allow(clippy::expect_used, clippy::unwrap_used)]

mod common;

use std::sync::Arc;

use common::{Harness, SERVER, USER_A};
use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::clock::FakeClock;
use strata_core::ids::SeqIds;
use strata_core::session::Core;
use strata_core::testing::{FakeAccountApi, FakeServer};
use strata_core::view::model::{Platform, SessionKind, SignInRequest, SignUpRequest};
use tempfile::TempDir;

fn insecure() -> CoreError {
    CoreError::InvalidInput {
        field: "server_url".to_owned(),
        reason: "insecure_http".to_owned(),
    }
}

fn sign_in(url: &str) -> SignInRequest {
    SignInRequest {
        server_url: url.to_owned(),
        username: "shawket".to_owned(),
        password: "pw-a".to_owned(),
        device_name: "Shawket's laptop".to_owned(),
    }
}

fn sign_up(url: &str) -> SignUpRequest {
    SignUpRequest {
        server_url: url.to_owned(),
        username: "nour".to_owned(),
        password: "a long enough password".to_owned(),
        display_name: "Nour".to_owned(),
    }
}

fn calls(h: &Harness) -> Vec<String> {
    h.accounts.calls.lock().unwrap().clone()
}

/// A core whose build default is `default`.
fn core_with_default(dir: &TempDir, default: Option<&str>) -> Core {
    let accounts = Arc::new(FakeAccountApi::default());
    let mut env = common::env(
        dir.path(),
        &FakeServer::new(),
        &accounts,
        &FakeClock::at(common::NOW),
        &Arc::new(SeqIds::new(common::ID_BASE_MS)),
        Platform::Android,
    );
    env.default_server_url = default.map(str::to_owned);
    Core::open(env).expect("core opens")
}

#[tokio::test]
async fn sign_in_refuses_plain_http_to_another_host_without_a_request() {
    let h = Harness::new();
    for url in ["http://strata.example", "http://187.124.33.153:8080/"] {
        let err = h.core.sign_in(sign_in(url)).await.expect_err(url);
        assert_eq!(err, insecure(), "{url}");
    }
    assert_eq!(calls(&h), Vec::<String>::new());
    let state = h.core.state().expect("state");
    assert_eq!(state.kind, SessionKind::SignedOut);
    // The refused address is not remembered: the form keeps the default.
    assert_eq!(state.server_url, Some(SERVER.to_owned()));
}

#[tokio::test]
async fn sign_up_refuses_plain_http_to_another_host_without_a_request() {
    let h = Harness::new();
    let err = h
        .core
        .sign_up(sign_up("HTTP://strata.example"))
        .await
        .expect_err("refused");
    assert_eq!(err, insecure());
    assert_eq!(calls(&h), Vec::<String>::new());
    assert_eq!(h.core.state().expect("state").kind, SessionKind::SignedOut);
}

#[tokio::test]
async fn sign_in_over_http_to_this_device_is_allowed() {
    for url in [
        "http://127.0.0.1:8080/",
        "http://localhost:8080",
        "http://[::1]:8080",
    ] {
        let h = Harness::new();
        let state = h.core.sign_in(sign_in(url)).await.expect(url);
        assert_eq!(state.kind, SessionKind::Active, "{url}");
        let account = state.account.expect("account");
        assert_eq!(account.user_id, USER_A);
        assert_eq!(account.server_url, url.trim_end_matches('/'), "{url}");
        assert_eq!(calls(&h)[0], "login:shawket");
    }
}

#[tokio::test]
async fn sign_up_stores_the_normalised_address() {
    let h = Harness::new();
    h.core
        .sign_up(sign_up(" https://strata.example/ "))
        .await
        .expect("sign up");
    let state = h.core.state().expect("state");
    assert_eq!(state.kind, SessionKind::PendingApproval);
    assert_eq!(state.server_url, Some(SERVER.to_owned()));
    assert_eq!(
        state.pending.expect("pending").server_url,
        SERVER.to_owned()
    );
}

#[test]
fn the_build_default_prefills_the_signed_out_session() {
    let dir = TempDir::new().expect("dir");
    let core = core_with_default(&dir, Some("https://strata.example"));
    assert_eq!(
        core.state().expect("state").server_url,
        Some("https://strata.example".to_owned())
    );
}

#[test]
fn without_a_build_default_the_field_stays_empty() {
    let dir = TempDir::new().expect("dir");
    let core = core_with_default(&dir, None);
    assert_eq!(core.state().expect("state").server_url, None);
}

#[test]
fn production_env_treats_a_blank_build_default_as_none() {
    let dir = TempDir::new().expect("dir");
    let config = |default: &str| strata_core::view::model::CoreConfig {
        app_data_dir: dir.path().to_string_lossy().into_owned(),
        platform: Platform::Linux,
        default_device_name: "laptop".to_owned(),
        default_server_url: Some(default.to_owned()),
    };
    let env = |default: &str| strata_core::session::CoreEnv::production(&config(default));
    assert_eq!(env("").default_server_url, None);
    assert_eq!(env("   ").default_server_url, None);
    assert_eq!(
        env(" https://strata.example/ ").default_server_url,
        Some("https://strata.example".to_owned())
    );
}
