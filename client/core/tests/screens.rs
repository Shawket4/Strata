//! Screen view-models the other suites leave out, built headlessly from a pulled vault (fake
//! server, fake clock, deterministic ULIDs): the task screen of a recurring task, task homes,
//! the Recent filters, the Home AI-activity feed, inbox summaries of every suggestion kind,
//! editor completions for block references and tags, and the sync-status screen.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::too_many_lines,
    dead_code
)]

mod common;

use std::sync::{Arc, Mutex};

use chrono::{DateTime, NaiveDate, Utc};
use common::{Harness, NOW, SERVER, USER_A};
use futures::future::BoxFuture;
use pretty_assertions::assert_eq;
use strata_core::net::{
    AccountApi, AdminUserInfo, AiDecisionInfo, MeInfo, NetError, SessionTokens, SimilarityEdge,
    SyncApi, Tokens,
};
use strata_core::session::{Core, CoreEnv, NotifyHub, Session};
use strata_core::store::StorePaths;
use strata_core::sync::engine::Trigger;
use strata_core::testing::{FakeAccountApi, FakeServer};
use strata_core::view::build;
use strata_core::view::extra;
use strata_core::view::model::{Platform, RecentFilter, SignInRequest};

const NOTE: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1B";
const TASKS: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1C";
const PERSON: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1D";
const COMPANY: &str = "01J8ZK3M4X7Q9W2E5R6T8Y0V1E";

fn show<T: std::fmt::Debug>(label: &str, v: &T) {
    println!("=== {label}\n{v:#?}");
}

/// A pulled vault: a note with a recurring task (one done occurrence) and a reminder, the
/// task home with an open task, a person with open items, a company.
async fn vault(h: &Harness) -> Arc<Session> {
    h.server.remote_upsert(
        NOTE,
        "notes/Rent.md",
        &format!(
            "---\nid: {NOTE}\ntags: [home]\n---\n# Rent\n\nPaid by transfer. ^how\n\n\
             - [ ] Pay rent (@2026-10-01 09:00) 🔁 every month 📅 2026-10-01 ^t-rent\n\
             - [x] Pay rent 🔁 every month 📅 2026-09-01 ✅ 2026-09-01 ^t-rent-sep\n\
             - [ ] Call the landlord #home 📅 2026-09-29 ^t-call\n"
        ),
    );
    h.server.remote_upsert(
        TASKS,
        "tasks/Tasks.md",
        &format!("---\nid: {TASKS}\n---\n- [ ] Buy milk ^t-milk\n"),
    );
    h.server.remote_upsert(
        PERSON,
        "people/Ahmed Fathy.md",
        &format!(
            "---\nid: {PERSON}\nkind: person\n---\n## Open items\n\n\
             - Send him the contract [[Rent#^how]]\n- Ask about the car\n"
        ),
    );
    h.server.remote_upsert(
        COMPANY,
        "companies/Acme.md",
        &format!("---\nid: {COMPANY}\nkind: company\n---\n"),
    );
    let s = h.sign_in_a().await;
    s.sync(Trigger::Start).await.expect("bootstrap");
    s
}

#[tokio::test]
async fn the_task_screen_of_a_recurring_task() {
    let h = Harness::new();
    let s = vault(&h).await;
    let screen = s
        .read(|c, ctx| build::task_screen(c, ctx, "t-rent"))
        .expect("screen");
    show("task screen", &screen);
    let missing = s
        .read(|c, ctx| build::task_screen(c, ctx, "t-none"))
        .expect("screen");
    show("missing", &missing);
    let homes = s.read(extra::task_homes).expect("homes");
    show("homes", &homes);
    for f in [
        RecentFilter::Edited,
        RecentFilter::Created,
        RecentFilter::FiledByAi,
    ] {
        let recent = s.read(|c, ctx| build::recent(c, ctx, f)).expect("recent");
        show("recent", &recent);
    }
    let home = s.read(build::home).expect("home");
    show("home", &home);
}

#[tokio::test]
async fn completions_for_block_references_and_tags() {
    let h = Harness::new();
    let s = vault(&h).await;
    for (content, cursor) in [
        ("See [[Rent#^", 12),
        ("See [[Rent#^h", 13),
        ("See [[Rent#^ho]] x", 14),
        ("See [[Nowhere#^", 15),
        ("Tag #ho", 7),
        ("plain text", 5),
    ] {
        let c = s
            .read(|c, ctx| extra::completions(c, ctx, NOTE, content, cursor))
            .expect("completions");
        show(content, &c);
    }
}
