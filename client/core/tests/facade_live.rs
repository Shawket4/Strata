//! The facade's background loop (`api::runtime`): `init_core` inside a Tokio runtime starts
//! the sync loop and the `/events` subscription of the active session. A write through the
//! facade reaches the server without an explicit sync; a change another device pushes reaches
//! the facade's core through `/events`; a disabled account closes the session.
//!
//! The core is a process-wide singleton, so this binary holds one test. Waiting is on
//! observable conditions only: the other device's `/events` stream, and the facade's own
//! one-shot reads re-checked (bounded) until the loop has pulled.
//!
//! Needs `PostgreSQL` (`STRATA_TEST_DATABASE_URL` or the testkit default).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

#[path = "support/world.rs"]
mod world;

use std::time::{Duration, Instant};

use pretty_assertions::assert_eq;
use strata_core::api::{app, intents, views};
use strata_core::net::EventSignal;
use strata_core::store::notes;
use strata_core::sync::engine::Trigger;
use strata_core::view::model::{
    AppLifecycle, CoreConfig, Platform, SearchMode, SessionKind, SignInRequest,
};
use strata_index::types::UserRole;
use world::World;

/// Re-checks `probe` until it holds (the background loop runs on its own task).
async fn eventually(what: &str, mut probe: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !probe() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn next_signal(stream: &mut Box<dyn strata_core::net::EventStream>) -> EventSignal {
    tokio::time::timeout(Duration::from_secs(30), stream.next())
        .await
        .expect("an event in time")
        .expect("stream open")
        .expect("event")
}

fn local_titles(query: &str) -> Vec<String> {
    futures::executor::block_on(views::search(query.to_owned(), SearchMode::Keyword))
        .map(|v| v.results.into_iter().map(|r| r.title).collect())
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_background_loop_pushes_pulls_and_follows_events() {
    let w = World::new().await;
    // The facade runs on the system clock; the server refuses creation times ahead of its
    // own, so its (fake) clock starts at the real time here.
    w.db.clock.set(chrono::Utc::now());
    let alice_id = w.account("alice", UserRole::Member).await;
    w.account("root", UserRole::Admin).await;
    let dir = tempfile::TempDir::new().expect("app data");
    let url = w.server.base_url();

    let state = app::init_core(CoreConfig {
        app_data_dir: dir.path().to_str().expect("utf-8").to_owned(),
        platform: Platform::Linux,
        default_device_name: "Live laptop".to_owned(),
        default_server_url: Some(url.clone()),
    })
    .await
    .expect("init");
    assert_eq!(state.kind, SessionKind::SignedOut);
    app::sync_now().expect("sync now without a session is a no-op");
    app::sign_in(SignInRequest {
        server_url: url.clone(),
        username: "alice".to_owned(),
        password: world::password("alice"),
        device_name: "Live laptop".to_owned(),
    })
    .await
    .expect("sign in");

    // Another device of Alice's watches the account's events.
    let phone = w.device(2);
    let other = phone.sign_in("alice").await;
    let mut events = other.subscribe_events().expect("subscribe");

    // A write through the facade is pushed by the loop (no explicit sync).
    let id = intents::create_note(
        "notes/Live.md".to_owned(),
        "Pushed by the loop.\n".to_owned(),
        false,
    )
    .expect("create")
    .id
    .expect("created");
    let signal = next_signal(&mut events).await;
    assert!(matches!(signal, EventSignal::Changed { .. }), "{signal:?}");
    other.sync(Trigger::EventsFrame).await.expect("pull");
    let pulled = other
        .read(|c, _| notes::current(c, &id))
        .expect("read")
        .expect("the phone has the note");
    assert!(
        pulled.content.ends_with("Pushed by the loop.\n"),
        "{}",
        pulled.content
    );

    // A change from the phone reaches the facade's core through its own `/events` loop.
    other
        .create_note("notes/From the phone.md", "Arrived by events.\n", false)
        .expect("create");
    other.sync(Trigger::AfterWrite).await.expect("push");
    eventually("the phone's note to arrive", || {
        local_titles("Arrived") == ["From the phone"]
    })
    .await;

    // Lifecycle and pause toggles wake the loop without breaking it.
    app::app_lifecycle(AppLifecycle::Resumed).expect("resume");
    app::app_lifecycle(AppLifecycle::Paused).expect("pause");
    app::set_sync_paused(true).expect("pause sync");
    app::set_sync_paused(false).expect("resume sync");

    // The admin disables the account: the events loop closes the session.
    let admin_device = w.device(3);
    let admin = admin_device.sign_in("root").await;
    admin
        .set_user_enabled(&alice_id, false)
        .await
        .expect("disable");
    eventually("the disabled screen", || {
        app::dismiss_pending().map(|s| s.kind) == Ok(SessionKind::Disabled)
    })
    .await;
    let signed_out = app::acknowledge_account_disabled().expect("acknowledge");
    assert_eq!(signed_out.kind, SessionKind::SignedOut);
    drop(other);
    w.finish().await;
}
