//! Login and sign-up rate limits (PLAN §8, §15): per IP and per username for login; per IP,
//! globally and with a pending-account cap for sign-up. Time is the fake clock.
#![allow(clippy::expect_used)] // tests: expect with messages

mod common;

use chrono::Duration;
use common::{Harness, assert_problem, plain};
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use strata_common::config::RateLimit;
use strata_index::types::UserRole;

fn limited(detail: &str) -> types::Problem {
    plain("rate_limited", "Too many requests", 429, Some(detail))
}

fn signup(name: &str) -> types::SignupRequest {
    types::SignupRequest {
        username: name.to_owned(),
        password: "some-password-1".to_owned(),
        display_name: name.to_owned(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn login_is_limited_per_username_including_confusable_spellings() {
    let h = Harness::with_config(|c| {
        c.auth.rate_limits.login_per_username = RateLimit {
            max: 3,
            window_secs: 60,
        };
    })
    .await;
    h.create_user("ahmed", "ahmed-password-1", UserRole::Member)
        .await;
    h.create_user("other", "other-password-1", UserRole::Member)
        .await;
    let wrong = plain("invalid_credentials", "Invalid username or password", 401, None);
    assert_problem(h.try_login("ahmed", "nope-nope-nope").await, &wrong);
    assert_problem(h.try_login("AHMED", "nope-nope-nope").await, &wrong);
    h.clock.advance(Duration::seconds(20));
    assert_problem(h.try_login("\u{0430}hmed", "nope-nope-nope").await, &wrong);
    // The fourth attempt within the window is refused, even with the right password.
    assert_problem(
        h.try_login("ahmed", "ahmed-password-1").await,
        &limited("too many login attempts for this account"),
    );
    // Other accounts are unaffected.
    h.login("other", "other-password-1").await;
    // Once the first attempts leave the window, login works again.
    h.clock.advance(Duration::seconds(40));
    let session = h.login("ahmed", "ahmed-password-1").await;
    assert!(!session.export_only);
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn login_is_limited_per_client_address() {
    let h = Harness::with_config(|c| {
        c.auth.rate_limits.login_per_ip = RateLimit {
            max: 2,
            window_secs: 300,
        };
    })
    .await;
    let wrong = plain("invalid_credentials", "Invalid username or password", 401, None);
    assert_problem(h.try_login("first", "x-password-123").await, &wrong);
    assert_problem(h.try_login("second", "x-password-123").await, &wrong);
    assert_problem(
        h.try_login("third", "x-password-123").await,
        &limited("too many login attempts from this address"),
    );
    h.clock.advance(Duration::seconds(300));
    assert_problem(h.try_login("third", "x-password-123").await, &wrong);
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rate_limited_responses_carry_retry_after() {
    let h = Harness::with_config(|c| {
        c.auth.rate_limits.login_per_ip = RateLimit {
            max: 1,
            window_secs: 90,
        };
    })
    .await;
    let body = rmp_serde::to_vec_named(&types::LoginRequest {
        username: "nobody".into(),
        password: "x-password-123".into(),
        device_name: "d".into(),
        platform: types::DevicePlatform::Android,
    })
    .expect("encode");
    let http = reqwest::Client::new();
    let send = || {
        http.post(format!("{}/api/v1/auth/login", h.base_url()))
            .header("content-type", "application/vnd.msgpack")
            .header("accept", "application/vnd.msgpack")
            .body(body.clone())
            .send()
    };
    assert_eq!(send().await.expect("first").status().as_u16(), 401);
    h.clock.advance(Duration::seconds(30));
    let limited = send().await.expect("second");
    assert_eq!(limited.status().as_u16(), 429);
    assert_eq!(
        limited
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok()),
        Some("60")
    );
    assert_eq!(
        limited
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/problem+msgpack")
    );
    let bytes = limited.bytes().await.expect("body");
    strata_api::contract::Contract::production()
        .validate_response("login", 429, Some("application/problem+msgpack"), &bytes)
        .expect("conformant");
    // The generated client is not used here, so the harness saw no responses; use it once.
    h.try_login("nobody", "x").await.expect_err("limited");
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn signup_is_limited_per_address_and_globally() {
    let h = Harness::with_config(|c| {
        c.auth.rate_limits.signup_per_ip = RateLimit {
            max: 2,
            window_secs: 3600,
        };
    })
    .await;
    let anon = h.anon();
    ops::signup(&anon, &signup("user1")).await.expect("1");
    ops::signup(&anon, &signup("user2")).await.expect("2");
    assert_problem(
        ops::signup(&anon, &signup("user3")).await,
        &limited("too many sign-ups from this address"),
    );
    h.clock.advance(Duration::hours(1));
    ops::signup(&anon, &signup("user3")).await.expect("3");
    h.finish().await;

    let h = Harness::with_config(|c| {
        c.auth.rate_limits.signup_global = RateLimit {
            max: 1,
            window_secs: 600,
        };
    })
    .await;
    let anon = h.anon();
    ops::signup(&anon, &signup("user1")).await.expect("1");
    assert_problem(
        ops::signup(&anon, &signup("user2")).await,
        &limited("too many sign-ups; try again later"),
    );
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn signups_stop_while_too_many_accounts_await_approval() {
    let h = Harness::with_config(|c| c.accounts.max_pending_signups = 2).await;
    let (_, admin) = h.admin().await;
    let anon = h.anon();
    let first = ops::signup(&anon, &signup("user1")).await.expect("1");
    ops::signup(&anon, &signup("user2")).await.expect("2");
    assert_problem(
        ops::signup(&anon, &signup("user3")).await,
        &limited("too many accounts are awaiting approval; try again later"),
    );
    ops::admin_approve_user(&admin, first.id)
        .await
        .expect("approve");
    ops::signup(&anon, &signup("user3")).await.expect("room again");
    assert_eq!(
        ops::admin_list_users(&admin, Some(&types::AccountStatus::Pending))
            .await
            .expect("list")
            .len(),
        2
    );
    h.finish().await;
}
