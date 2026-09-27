//! Account deletion (D25, PLAN §5.2, §16.3 "Account deletion"): scheduling revokes sessions,
//! login yields an export-only session, every other endpoint is `403
//! account_deletion_pending`, the export holds exactly the user's vault, and the purge (grace
//! period over, or confirmed) removes the directory and every row of that user only, with one
//! audit entry. Cancelling restores the account.
#![allow(clippy::expect_used, clippy::too_many_lines)] // tests: expect with messages, long flows

mod common;

use std::collections::BTreeMap;
use std::io::Read;

use chrono::Duration;
use common::{Harness, assert_problem, plain, unauthorized};
use pretty_assertions::assert_eq;
use strata_api::auth::purge_due_accounts;
use strata_client::{Method, Request, operations as ops, types};
use strata_common::UserId;
use strata_index::types::UserRole;

/// Rows per table owned by `user` (superuser: bypasses RLS), including the `users` row.
async fn rows_of(h: &Harness, user: UserId) -> BTreeMap<String, i64> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.columns \
         WHERE table_schema = 'strata' AND column_name = 'user_id' ORDER BY 1",
    )
    .fetch_all(&h.db.superuser)
    .await
    .expect("tables");
    let mut out = BTreeMap::new();
    for table in tables {
        let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*) FROM strata.\"{table}\" WHERE user_id = $1"
        )))
        .bind(user)
        .fetch_one(&h.db.superuser)
        .await
        .expect("count");
        if n > 0 {
            out.insert(table, n);
        }
    }
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM strata.users WHERE id = $1")
        .bind(user)
        .fetch_one(&h.db.superuser)
        .await
        .expect("count");
    if users > 0 {
        out.insert("users".to_owned(), users);
    }
    out
}

fn zip_entries(bytes: &[u8]) -> BTreeMap<String, String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("a zip");
    let mut out = BTreeMap::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).expect("entry");
        let mut content = String::new();
        file.read_to_string(&mut content).expect("utf-8");
        out.insert(file.name().to_owned(), content);
    }
    out
}

fn write(h: &Harness, user: UserId, path: &str, content: &str) {
    let full = h.vault_dir(user).join(path);
    std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
    std::fs::write(full, content).expect("write");
}

async fn purge_audit(h: &Harness) -> Vec<(Option<UserId>, String, String)> {
    h.db.accounts_db
        .list_audit(1000)
        .await
        .expect("audit")
        .into_iter()
        .filter(|e| e.action == "user.purge")
        .map(|e| (e.actor_id, e.action, e.target))
        .collect()
}

fn deletion_pending() -> types::Problem {
    plain(
        "account_deletion_pending",
        "Account scheduled for deletion",
        403,
        Some("this session can only export the vault, confirm the deletion or sign out"),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn scheduled_deletion_export_only_session_and_purge_after_the_grace_period() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let alice = h
        .create_user("alice", "alice-password-1", UserRole::Member)
        .await;
    let bob = h.create_user("bob", "bob-password-1", UserRole::Member).await;
    write(&h, alice, "notes/Plan.md", "# Alice's plan\n");
    write(&h, alice, "people/أحمد.md", "عربي\n");
    write(&h, bob, "notes/Plan.md", "# Bob's plan\n");
    let alice_before = h.login("alice", "alice-password-1").await;
    let bob_session = h.login("bob", "bob-password-1").await;
    for session in [&alice_before, &bob_session] {
        ops::update_me(
            &h.with_token(&session.access_token),
            &types::UpdateMe {
                timezone: Some("Africa/Cairo".into()),
                ..Default::default()
            },
        )
        .await
        .expect("settings");
    }
    let bob_rows = rows_of(&h, bob).await;
    assert_eq!(
        bob_rows,
        BTreeMap::from([
            ("devices".to_owned(), 1),
            ("refresh_tokens".to_owned(), 1),
            ("sessions".to_owned(), 1),
            ("settings".to_owned(), 1),
            ("users".to_owned(), 1),
        ])
    );

    // Schedule: sessions end now, the purge is due after the grace period.
    let scheduled = ops::admin_delete_user(&admin, alice.as_ulid())
        .await
        .expect("schedule");
    assert_eq!(scheduled.status, types::AccountStatus::DeletionPending);
    assert_eq!(scheduled.deletion_requested_at, Some(h.now()));
    assert_eq!(scheduled.deletion_at, Some(h.now() + Duration::days(14)));
    assert_eq!(scheduled.export_downloaded_at, None);
    assert_problem(
        ops::get_me(&h.with_token(&alice_before.access_token)).await,
        &unauthorized("session revoked"),
    );

    // Login gives an export-only session.
    let export_session = h.login("alice", "alice-password-1").await;
    assert!(export_session.export_only);
    let client = h.with_token(&export_session.access_token);
    assert_problem(ops::list_devices(&client).await, &deletion_pending());
    assert_problem(
        ops::update_me(&client, &types::UpdateMe::default()).await,
        &deletion_pending(),
    );
    assert_problem(
        ops::delete_device(&client, export_session.device_id).await,
        &deletion_pending(),
    );
    assert_problem(ops::admin_list_users(&client, None).await, &deletion_pending());
    let me = ops::get_me(&client).await.expect("me");
    assert_eq!(me.status, types::AccountStatus::DeletionPending);
    assert!(me.export_only);
    assert_eq!(me.deletion_at, Some(h.now() + Duration::days(14)));
    // Refreshing keeps the session export-only.
    let refreshed = ops::refresh(
        &h.anon(),
        &types::RefreshRequest {
            refresh_token: export_session.refresh_token.clone(),
        },
    )
    .await
    .expect("refresh");
    assert!(refreshed.export_only);
    let client = h.with_token(&refreshed.access_token);
    assert_problem(ops::list_devices(&client).await, &deletion_pending());

    // The export holds exactly Alice's vault (no git history, nothing of Bob's).
    let zip = ops::export_me(&client).await.expect("export");
    assert_eq!(
        zip_entries(&zip),
        BTreeMap::from([
            ("notes/Plan.md".to_owned(), "# Alice's plan\n".to_owned()),
            ("people/أحمد.md".to_owned(), "عربي\n".to_owned()),
        ])
    );
    // Admins see that it was downloaded, never the export itself.
    let listed = ops::admin_list_users(&admin, Some(&types::AccountStatus::DeletionPending))
        .await
        .expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].export_downloaded_at, Some(h.now()));
    let admin_zip = ops::export_me(&admin).await.expect("admin's own export");
    assert_eq!(zip_entries(&admin_zip), BTreeMap::new());
    let probe = h.unchecked_with_token(&h.login("admin", "admin-password-1").await.access_token);
    for path in [
        format!("/api/v1/admin/users/{alice}/export"),
        format!("/api/v1/users/{alice}/export"),
        format!("/api/v1/me/export/{alice}"),
    ] {
        let err = probe
            .send_zip(Request::new(Method::GET, path, "probe").authenticated())
            .await
            .expect_err("no such route");
        assert_eq!(
            common::problem(&err),
            plain("route_not_found", "No such route", 404, None)
        );
    }

    // Not due yet one second before the end of the grace period.
    h.clock.advance(Duration::days(14) - Duration::seconds(1));
    assert_eq!(
        purge_due_accounts(&h.state, h.now()).await.expect("purge"),
        vec![]
    );
    assert!(h.vault_dir(alice).exists());
    // A token issued just before the purge, still unexpired after it.
    let last = ops::refresh(
        &h.anon(),
        &types::RefreshRequest {
            refresh_token: refreshed.refresh_token.clone(),
        },
    )
    .await
    .expect("refresh");
    let client = h.with_token(&last.access_token);
    h.clock.advance(Duration::seconds(1));
    assert_eq!(
        purge_due_accounts(&h.state, h.now()).await.expect("purge"),
        vec![alice]
    );

    // Directory and every row gone — Alice's only.
    assert!(!h.data.path().join("users").join(alice.to_string()).exists());
    assert_eq!(rows_of(&h, alice).await, BTreeMap::new());
    assert_eq!(rows_of(&h, bob).await, bob_rows);
    assert_eq!(
        std::fs::read_to_string(h.vault_dir(bob).join("notes/Plan.md")).expect("bob intact"),
        "# Bob's plan\n"
    );
    assert_eq!(
        purge_audit(&h).await,
        vec![(None, "user.purge".to_owned(), format!("user:{alice}"))]
    );
    assert!(
        h.db.accounts_db
            .list_audit(1000)
            .await
            .expect("audit")
            .iter()
            .any(|e| e.actor_id == Some(admin_id)
                && e.action == "user.delete.schedule"
                && e.target == format!("user:{alice}"))
    );

    // Every token of Alice's dies; the account no longer exists.
    assert_problem(ops::get_me(&client).await, &unauthorized("account deleted"));
    h.state.reload_revocations().await.expect("reload");
    assert_problem(ops::get_me(&client).await, &unauthorized("account deleted"));
    assert_problem(
        h.try_login("alice", "alice-password-1").await,
        &plain("invalid_credentials", "Invalid username or password", 401, None),
    );
    // A second run has nothing to do.
    assert_eq!(
        purge_due_accounts(&h.state, h.now()).await.expect("purge"),
        vec![]
    );
    let bob_again = ops::refresh(
        &h.anon(),
        &types::RefreshRequest {
            refresh_token: bob_session.refresh_token.clone(),
        },
    )
    .await
    .expect("bob's session is unaffected");
    ops::get_me(&h.with_token(&bob_again.access_token))
        .await
        .expect("bob unaffected");
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn confirming_the_deletion_purges_immediately() {
    let h = Harness::new().await;
    let (_, admin) = h.admin().await;
    let carl = h
        .create_user("carl", "carl-password-1", UserRole::Member)
        .await;
    write(&h, carl, "notes/a.md", "a");
    // Confirming is only possible while a deletion is scheduled.
    let active = h.with_token(&h.login("carl", "carl-password-1").await.access_token);
    assert_problem(
        ops::confirm_deletion(&active).await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("the account is not scheduled for deletion"),
        ),
    );
    ops::admin_delete_user(&admin, carl.as_ulid())
        .await
        .expect("schedule");
    let session = h.login("carl", "carl-password-1").await;
    let client = h.with_token(&session.access_token);
    ops::confirm_deletion(&client).await.expect("confirm");
    assert!(!h.data.path().join("users").join(carl.to_string()).exists());
    assert_eq!(rows_of(&h, carl).await, BTreeMap::new());
    assert_eq!(
        purge_audit(&h).await,
        vec![(None, "user.purge".to_owned(), format!("user:{carl}"))]
    );
    assert_problem(ops::get_me(&client).await, &unauthorized("account deleted"));
    assert_eq!(
        ops::admin_list_users(&admin, None)
            .await
            .expect("list")
            .iter()
            .map(|u| u.username.as_str())
            .collect::<Vec<_>>(),
        vec!["admin"]
    );
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelling_a_deletion_restores_the_account() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let dana = h
        .create_user("dana", "dana-password-1", UserRole::Member)
        .await;
    write(&h, dana, "notes/keep.md", "keep");
    ops::admin_delete_user(&admin, dana.as_ulid())
        .await
        .expect("schedule");
    let export_only = h.login("dana", "dana-password-1").await;
    assert!(export_only.export_only);

    let restored = ops::admin_cancel_deletion(&admin, dana.as_ulid())
        .await
        .expect("cancel");
    assert_eq!(restored.status, types::AccountStatus::Active);
    assert_eq!(restored.deletion_at, None);
    assert_eq!(restored.deletion_requested_at, None);
    // The export-only session ends; a normal login works again.
    assert_problem(
        ops::get_me(&h.with_token(&export_only.access_token)).await,
        &unauthorized("session revoked"),
    );
    let session = h.login("dana", "dana-password-1").await;
    assert!(!session.export_only);
    let client = h.with_token(&session.access_token);
    ops::list_devices(&client).await.expect("full access");
    assert_eq!(
        ops::get_me(&client).await.expect("me").status,
        types::AccountStatus::Active
    );
    assert_problem(
        ops::admin_cancel_deletion(&admin, dana.as_ulid()).await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("no deletion is scheduled"),
        ),
    );
    // Nothing is purged later.
    h.clock.advance(Duration::days(15));
    assert_eq!(
        purge_due_accounts(&h.state, h.now()).await.expect("purge"),
        vec![]
    );
    assert!(h.vault_dir(dana).join("notes/keep.md").is_file());
    assert!(
        h.db.accounts_db
            .list_audit(1000)
            .await
            .expect("audit")
            .iter()
            .any(|e| e.actor_id == Some(admin_id)
                && e.action == "user.delete.cancel"
                && e.target == format!("user:{dana}"))
    );
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_disabled_account_scheduled_for_deletion_can_still_export() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let eve = h.create_user("eve", "eve-password-1", UserRole::Member).await;
    ops::admin_update_user(
        &admin,
        eve.as_ulid(),
        &types::UpdateUser {
            status: Some(types::SettableStatus::Disabled),
            ..Default::default()
        },
    )
    .await
    .expect("disable");
    ops::admin_delete_user(&admin, eve.as_ulid())
        .await
        .expect("schedule");
    let session = h.login("eve", "eve-password-1").await;
    assert!(session.export_only);
    ops::export_me(&h.with_token(&session.access_token))
        .await
        .expect("export");
    // Admins cannot delete themselves, and only active/disabled accounts can be scheduled.
    assert_problem(
        ops::admin_delete_user(&admin, admin_id.as_ulid()).await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("admins cannot delete their own account"),
        ),
    );
    assert_problem(
        ops::admin_delete_user(&admin, eve.as_ulid()).await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("only active or disabled accounts can be scheduled for deletion"),
        ),
    );
    h.finish().await;
}
