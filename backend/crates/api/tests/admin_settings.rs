//! `GET /admin/settings` (D25): an admin sees the deletion grace period before scheduling a
//! deletion, and the purge date `DELETE /admin/users/{id}` then sets is exactly now plus it.
//! Members get `403 forbidden`. Every response is validated against the contract.
#![allow(clippy::expect_used)] // tests: expect with messages

mod common;

use chrono::Duration;
use common::{Harness, assert_problem, plain};
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use strata_common::Clock;
use strata_index::types::UserRole;

#[tokio::test(flavor = "multi_thread")]
async fn admins_read_the_grace_period_and_scheduling_uses_it() {
    let h = Harness::with_config(|c| c.accounts.deletion_grace_days = 9).await;
    let (_, admin) = h.admin().await;
    let alice = h
        .create_user("alice", "alice-password-1", UserRole::Member)
        .await;
    assert_eq!(
        ops::admin_settings(&admin).await.expect("settings"),
        types::AdminSettings {
            deletion_grace_secs: 9 * 24 * 3600,
        }
    );
    let now = h.clock.now();
    let scheduled = ops::admin_delete_user(&admin, alice.as_ulid())
        .await
        .expect("scheduled");
    assert_eq!(scheduled.deletion_at, Some(now + Duration::days(9)));
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn members_are_forbidden() {
    let h = Harness::new().await;
    h.create_user("bob", "bob-password-1", UserRole::Member)
        .await;
    let bob = h.login("bob", "bob-password-1").await;
    assert_problem(
        ops::admin_settings(&h.with_token(&bob.access_token)).await,
        &plain("forbidden", "Forbidden", 403, None),
    );
    // The default grace period is 14 days.
    let (_, admin) = h.admin().await;
    assert_eq!(
        ops::admin_settings(&admin)
            .await
            .expect("settings")
            .deletion_grace_secs,
        14 * 24 * 3600
    );
    h.finish().await;
}
