//! The online intents of the client core against the real server (PLAN §12.6, §12.7): devices,
//! the account (`PATCH /me`, export, deletion), note history and revert, server search, vault
//! export and import, Admin → Users, the AI activity feed, similarity, saved map layouts and
//! Ask — each through the generated client to the production app served in process, with the
//! results the screens show asserted exactly.
//!
//! Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL` or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

#[path = "support/world.rs"]
mod world;

use pretty_assertions::assert_eq;
use strata_core::CoreError;
use strata_core::store::notes;
use strata_core::sync::engine::Trigger;
use strata_core::view::build;
use strata_index::types::UserRole;
use world::World;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn devices_are_listed_renamed_muted_and_revoked() {
    let w = World::new().await;
    w.account("alice", UserRole::Member).await;
    let laptop = w.device(1);
    let phone = w.device(2);
    let s1 = laptop.sign_in("alice").await;
    let s2 = phone.sign_in("alice").await;

    s1.refresh_settings().await.expect("refresh");
    let view = s1
        .read(|c, ctx| build::settings(c, ctx))
        .expect("settings");
    println!("{view:#?}");
    w.finish().await;
}
