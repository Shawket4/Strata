//! Sign-up, approval, login, refresh rotation and reuse detection, logout, disable, password
//! change/reset and role changes — through the generated client against the real app, every
//! response validated against the contract (PLAN §7.5, §8, §16.3 "Accounts").
#![allow(clippy::expect_used, clippy::too_many_lines)] // tests: expect with messages, long flows

mod common;

use chrono::Duration;
use common::{Harness, assert_problem, invalid_field, plain, problem, unauthorized};
use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use strata_index::types::UserRole;

fn signup_body(username: &str) -> types::SignupRequest {
    types::SignupRequest {
        username: username.to_owned(),
        password: "member-password-1".to_owned(),
        display_name: "Sam Member".to_owned(),
    }
}

fn refresh_body(token: &str) -> types::RefreshRequest {
    types::RefreshRequest {
        refresh_token: token.to_owned(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn signup_approval_login_refresh_reuse_and_logout() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let anon = h.anon();

    // Sign-up: pending, no vault, no session.
    let pending = ops::signup(&anon, &signup_body("Sam"))
        .await
        .expect("signup");
    assert_eq!(
        pending,
        types::PendingAccount {
            display_name: "Sam Member".into(),
            id: pending.id,
            status: types::AccountStatus::Pending,
            username: "Sam".into(),
        }
    );
    let sam = strata_common::UserId::from_ulid(pending.id);
    assert_problem(
        h.try_login("sam", "member-password-1").await,
        &plain("account_pending", "Account awaiting approval", 403, None),
    );
    // A wrong password never reveals the status.
    assert_problem(
        h.try_login("sam", "wrong-password-1").await,
        &plain(
            "invalid_credentials",
            "Invalid username or password",
            401,
            None,
        ),
    );
    assert!(!h.data.path().join("users").join(sam.to_string()).exists());

    // Only admins approve.
    h.create_user("mona", "mona-password-1", UserRole::Member)
        .await;
    let mona = h.login("mona", "mona-password-1").await;
    assert_problem(
        ops::admin_approve_user(&h.with_token(&mona.access_token), pending.id).await,
        &plain("forbidden", "Forbidden", 403, None),
    );

    // Approval creates the vault (a git repository) exactly once.
    let approved = ops::admin_approve_user(&admin, pending.id)
        .await
        .expect("approve");
    assert_eq!(approved.status, types::AccountStatus::Active);
    assert_eq!(approved.approved_at, Some(h.now()));
    assert_eq!(approved.username, "Sam");
    let vault = h.vault_dir(sam);
    assert_eq!(
        std::fs::read_to_string(vault.join(".git/HEAD")).expect("git repo"),
        "ref: refs/heads/main\n"
    );
    std::fs::write(vault.join("marker.md"), "kept").expect("marker");
    assert_problem(
        ops::admin_approve_user(&admin, pending.id).await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("the account is not pending"),
        ),
    );
    assert_eq!(
        std::fs::read_to_string(vault.join("marker.md")).expect("marker"),
        "kept"
    );
    let user_dirs: Vec<String> = std::fs::read_dir(h.data.path().join("users"))
        .expect("users dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n == &sam.to_string())
        .collect();
    assert_eq!(user_dirs.len(), 1);

    // Login (case-insensitive username) → a device session.
    let s1 = h.login("SAM", "member-password-1").await;
    assert_eq!(s1.user_id, pending.id);
    assert!(!s1.export_only);
    assert!(!s1.password_change_required);
    assert_eq!(s1.access_token_expires_at, h.now() + Duration::minutes(15));
    assert_eq!(s1.refresh_token_expires_at, h.now() + Duration::days(90));
    let me = ops::get_me(&h.with_token(&s1.access_token))
        .await
        .expect("me");
    assert_eq!(
        me,
        types::Me {
            created: me.created,
            deletion_at: None,
            display_name: "Sam Member".into(),
            export_only: false,
            id: pending.id,
            password_change_required: false,
            preferences: std::collections::HashMap::default(),
            role: types::Role::Member,
            status: types::AccountStatus::Active,
            timezone: "UTC".into(),
            ui_language: types::UiLanguage::En,
            username: "Sam".into(),
        }
    );

    // Refresh rotates: a new pair, same session and device.
    h.clock.advance(Duration::minutes(10));
    let s2 = ops::refresh(&anon, &refresh_body(&s1.refresh_token))
        .await
        .expect("refresh");
    assert_eq!(
        (s2.session_id, s2.device_id, s2.user_id),
        (s1.session_id, s1.device_id, s1.user_id)
    );
    assert_ne!(s2.refresh_token, s1.refresh_token);
    assert_eq!(s2.access_token_expires_at, h.now() + Duration::minutes(15));
    ops::get_me(&h.with_token(&s2.access_token))
        .await
        .expect("new access token works");

    // Replaying the spent refresh token revokes the whole device session, immediately.
    assert_problem(
        ops::refresh(&anon, &refresh_body(&s1.refresh_token)).await,
        &unauthorized("refresh token reuse detected; the session was revoked"),
    );
    assert_problem(
        ops::get_me(&h.with_token(&s2.access_token)).await,
        &unauthorized("session revoked"),
    );
    assert_problem(
        ops::refresh(&anon, &refresh_body(&s2.refresh_token)).await,
        &unauthorized("invalid or expired refresh token"),
    );

    // A fresh login, then logout ends that session only.
    let s3 = h.login("sam", "member-password-1").await;
    let s4 = h.login("sam", "member-password-1").await;
    ops::logout(&h.with_token(&s3.access_token))
        .await
        .expect("logout");
    assert_problem(
        ops::get_me(&h.with_token(&s3.access_token)).await,
        &unauthorized("session revoked"),
    );
    assert_problem(
        ops::refresh(&anon, &refresh_body(&s3.refresh_token)).await,
        &unauthorized("invalid or expired refresh token"),
    );
    ops::get_me(&h.with_token(&s4.access_token))
        .await
        .expect("other session unaffected");

    // Expired access token.
    h.clock.advance(Duration::minutes(16));
    assert_problem(
        ops::get_me(&h.with_token(&s4.access_token)).await,
        &unauthorized("access token expired"),
    );
    // No token at all.
    assert_problem(
        ops::get_me(&anon).await,
        &unauthorized("a bearer access token is required"),
    );

    // Audit entries for the admin actions.
    let audit = h.state_audit().await;
    assert!(audit.contains(&(
        Some(admin_id),
        "user.approve".to_owned(),
        format!("user:{sam}")
    )));
    assert!(audit.contains(&(Some(sam), "user.signup".to_owned(), format!("user:{sam}"))));
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_signups_cannot_log_in() {
    let h = Harness::new().await;
    let (_, admin) = h.admin().await;
    let pending = ops::signup(&h.anon(), &signup_body("rita"))
        .await
        .expect("signup");
    let rejected = ops::admin_reject_user(&admin, pending.id)
        .await
        .expect("reject");
    assert_eq!(rejected.status, types::AccountStatus::Rejected);
    assert_eq!(rejected.rejected_at, Some(h.now()));
    assert_problem(
        h.try_login("rita", "member-password-1").await,
        &plain("account_rejected", "Account rejected", 403, None),
    );
    assert_problem(
        ops::admin_approve_user(&admin, pending.id).await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("the account is not pending"),
        ),
    );
    assert!(
        !h.data
            .path()
            .join("users")
            .join(pending.id.to_string())
            .exists()
    );
    // Unknown IDs are 404.
    assert_problem(
        ops::admin_reject_user(&admin, ulid::Ulid::from_parts(1, 1)).await,
        &plain("not_found", "Not found", 404, None),
    );
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn disabling_revokes_live_tokens_immediately() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let member = h
        .create_user("dina", "dina-password-1", UserRole::Member)
        .await;
    let session = h.login("dina", "dina-password-1").await;
    let client = h.with_token(&session.access_token);
    ops::get_me(&client).await.expect("works before");

    let result = ops::admin_update_user(
        &admin,
        member.as_ulid(),
        &types::UpdateUser {
            status: Some(types::SettableStatus::Disabled),
            ..Default::default()
        },
    )
    .await
    .expect("disable");
    assert_eq!(result.user.status, types::AccountStatus::Disabled);
    assert_eq!(result.user.disabled_at, Some(h.now()));
    assert_eq!(result.temporary_password, None);

    // The still-unexpired access token is refused on the very next request.
    assert_problem(
        ops::get_me(&client).await,
        &unauthorized("account disabled"),
    );
    assert_problem(
        ops::refresh(&h.anon(), &refresh_body(&session.refresh_token)).await,
        &unauthorized("invalid or expired refresh token"),
    );
    assert_problem(
        h.try_login("dina", "dina-password-1").await,
        &plain("account_disabled", "Account disabled", 403, None),
    );
    // Survives a reload from the database.
    h.state.reload_revocations().await.expect("reload");
    assert_problem(
        ops::get_me(&client).await,
        &unauthorized("account disabled"),
    );

    // Admins cannot disable themselves; enabling only applies to disabled accounts.
    assert_problem(
        ops::admin_update_user(
            &admin,
            admin_id.as_ulid(),
            &types::UpdateUser {
                status: Some(types::SettableStatus::Disabled),
                ..Default::default()
            },
        )
        .await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("admins cannot disable their own account"),
        ),
    );
    let enabled = ops::admin_update_user(
        &admin,
        member.as_ulid(),
        &types::UpdateUser {
            status: Some(types::SettableStatus::Active),
            ..Default::default()
        },
    )
    .await
    .expect("enable");
    assert_eq!(enabled.user.status, types::AccountStatus::Active);
    assert_eq!(enabled.user.disabled_at, None);
    // Old sessions stay revoked; a new login works.
    assert_problem(ops::get_me(&client).await, &unauthorized("session revoked"));
    let fresh = h.login("dina", "dina-password-1").await;
    ops::get_me(&h.with_token(&fresh.access_token))
        .await
        .expect("works after enable");
    assert_problem(
        ops::admin_update_user(
            &admin,
            member.as_ulid(),
            &types::UpdateUser {
                status: Some(types::SettableStatus::Active),
                ..Default::default()
            },
        )
        .await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("only disabled accounts can be enabled"),
        ),
    );
    let audit = h.state_audit().await;
    for action in ["user.disable", "user.enable"] {
        assert!(audit.contains(&(Some(admin_id), action.to_owned(), format!("user:{member}"))));
    }
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn password_change_requires_the_current_password_and_signs_out_other_devices() {
    let h = Harness::new().await;
    h.create_user("paul", "paul-password-1", UserRole::Member)
        .await;
    let phone = h.login("paul", "paul-password-1").await;
    let laptop = h.login("paul", "paul-password-1").await;
    let client = h.with_token(&laptop.access_token);

    let change = |current: Option<&str>, new: &str| types::UpdateMe {
        current_password: current.map(str::to_owned),
        new_password: Some(new.to_owned()),
        ..Default::default()
    };
    assert_problem(
        ops::update_me(&client, &change(None, "paul-password-2")).await,
        &invalid_field(
            "current_password_required",
            "/current_password",
            "changing the password requires current_password",
        ),
    );
    assert_problem(
        ops::update_me(&client, &change(Some("nope-nope-nope"), "paul-password-2")).await,
        &invalid_field(
            "current_password_mismatch",
            "/current_password",
            "current_password is not the account's password",
        ),
    );
    assert_problem(
        ops::update_me(&client, &change(Some("paul-password-1"), "short")).await,
        &invalid_field(
            "invalid_password",
            "/new_password",
            "passwords have at least 10 characters and at most 1024 bytes",
        ),
    );
    let me = ops::update_me(&client, &change(Some("paul-password-1"), "paul-password-2"))
        .await
        .expect("changed");
    assert!(!me.password_change_required);
    assert_eq!(stored().await, (true, false));
    // This device keeps working; the other one is signed out immediately.
    ops::get_me(&client).await.expect("current session kept");
    assert_problem(
        ops::get_me(&h.with_token(&phone.access_token)).await,
        &unauthorized("session revoked"),
    );
    assert_problem(
        h.try_login("paul", "paul-password-1").await,
        &plain(
            "invalid_credentials",
            "Invalid username or password",
            401,
            None,
        ),
    );
    h.login("paul", "paul-password-2").await;
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_password_reset_forces_a_change_at_next_login() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let member = h
        .create_user("rana", "rana-password-1", UserRole::Member)
        .await;
    let before = h.login("rana", "rana-password-1").await;

    let reset = ops::admin_update_user(
        &admin,
        member.as_ulid(),
        &types::UpdateUser {
            reset_password: Some(true),
            ..Default::default()
        },
    )
    .await
    .expect("reset");
    let temporary = reset
        .temporary_password
        .clone()
        .expect("temporary password");
    assert_eq!(temporary.chars().count(), 16);
    assert!(reset.user.password_change_required);
    // The flag is a column; the hash is a plain PHC string.
    let stored = || async {
        let (hash, flag): (String, bool) =
            sqlx::query_as("SELECT password_hash, must_change_password FROM users WHERE id = $1")
                .bind(member)
                .fetch_one(&h.db.accounts)
                .await
                .expect("user row");
        (hash.starts_with("$argon2id$"), flag)
    };
    assert_eq!(stored().await, (true, true));
    assert_problem(
        ops::get_me(&h.with_token(&before.access_token)).await,
        &unauthorized("session revoked"),
    );
    assert_problem(
        h.try_login("rana", "rana-password-1").await,
        &plain(
            "invalid_credentials",
            "Invalid username or password",
            401,
            None,
        ),
    );

    let session = h.login("rana", &temporary).await;
    assert!(session.password_change_required);
    let client = h.with_token(&session.access_token);
    let restricted = plain(
        "password_change_required",
        "Password change required",
        403,
        Some("set a new password with PATCH /me before using the API"),
    );
    assert_problem(ops::list_devices(&client).await, &restricted);
    assert_problem(ops::export_me(&client).await, &restricted);
    let me = ops::get_me(&client).await.expect("me allowed");
    assert!(me.password_change_required);
    let me = ops::update_me(
        &client,
        &types::UpdateMe {
            current_password: Some(temporary.clone()),
            new_password: Some("rana-password-2".into()),
            ..Default::default()
        },
    )
    .await
    .expect("changed");
    assert!(!me.password_change_required);
    assert_eq!(stored().await, (true, false));
    // The device from before the reset is still listed (its session ended).
    assert_eq!(
        ops::list_devices(&client)
            .await
            .expect("unrestricted")
            .len(),
        2
    );
    assert_problem(
        h.try_login("rana", &temporary).await,
        &plain(
            "invalid_credentials",
            "Invalid username or password",
            401,
            None,
        ),
    );
    let audit = h.state_audit().await;
    assert!(audit.contains(&(
        Some(admin_id),
        "user.password_reset".to_owned(),
        format!("user:{member}")
    )));
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn role_changes_apply_on_the_next_request() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let member = h
        .create_user("omar", "omar-password-1", UserRole::Member)
        .await;
    let omar = h.with_token(&h.login("omar", "omar-password-1").await.access_token);
    assert_problem(
        ops::admin_list_users(&omar, None).await,
        &plain("forbidden", "Forbidden", 403, None),
    );
    let promoted = ops::admin_update_user(
        &admin,
        member.as_ulid(),
        &types::UpdateUser {
            role: Some(types::Role::Admin),
            ..Default::default()
        },
    )
    .await
    .expect("promote");
    assert_eq!(promoted.user.role, types::Role::Admin);
    // Same access token, new role (admin routes check the database).
    let users = ops::admin_list_users(&omar, None).await.expect("admin now");
    assert_eq!(
        users
            .iter()
            .map(|u| u.username.as_str())
            .collect::<Vec<_>>(),
        vec!["admin", "omar"]
    );
    ops::admin_update_user(
        &admin,
        member.as_ulid(),
        &types::UpdateUser {
            role: Some(types::Role::Member),
            ..Default::default()
        },
    )
    .await
    .expect("demote");
    assert_problem(
        ops::admin_list_users(&omar, None).await,
        &plain("forbidden", "Forbidden", 403, None),
    );
    assert_problem(
        ops::admin_update_user(
            &admin,
            admin_id.as_ulid(),
            &types::UpdateUser {
                role: Some(types::Role::Member),
                ..Default::default()
            },
        )
        .await,
        &plain(
            "account_state_conflict",
            "Account state does not allow this",
            409,
            Some("admins cannot demote themselves"),
        ),
    );
    let audit = h.state_audit().await;
    for action in ["user.role.admin", "user.role.member"] {
        assert!(audit.contains(&(Some(admin_id), action.to_owned(), format!("user:{member}"))));
    }
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn admin_created_accounts_are_active_with_a_vault() {
    let h = Harness::new().await;
    let (admin_id, admin) = h.admin().await;
    let created = ops::admin_create_user(
        &admin,
        &types::CreateUser {
            display_name: "Nour".into(),
            password: "nour-password-1".into(),
            role: types::Role::Member,
            username: "nour".into(),
        },
    )
    .await
    .expect("create");
    assert_eq!(
        created,
        types::AdminUser {
            approved_at: Some(h.now()),
            created: h.now(),
            deletion_at: None,
            deletion_requested_at: None,
            disabled_at: None,
            display_name: "Nour".into(),
            export_downloaded_at: None,
            id: created.id,
            password_change_required: false,
            rejected_at: None,
            role: types::Role::Member,
            status: types::AccountStatus::Active,
            username: "nour".into(),
        }
    );
    let nour = strata_common::UserId::from_ulid(created.id);
    assert!(h.vault_dir(nour).join(".git/HEAD").is_file());
    h.login("nour", "nour-password-1").await;
    let active = ops::admin_list_users(&admin, Some(&types::AccountStatus::Active))
        .await
        .expect("list");
    assert_eq!(active.len(), 2);
    let pending = ops::admin_list_users(&admin, Some(&types::AccountStatus::Pending))
        .await
        .expect("list");
    assert_eq!(pending, vec![]);
    assert_problem(
        ops::admin_create_user(
            &admin,
            &types::CreateUser {
                display_name: "x".into(),
                password: "p".into(),
                role: types::Role::Member,
                username: "x".into(),
            },
        )
        .await,
        &invalid_field(
            "invalid_username",
            "/username",
            "usernames have 3 to 32 characters",
        ),
    );
    let audit = h.state_audit().await;
    assert!(audit.contains(&(
        Some(admin_id),
        "user.create".to_owned(),
        format!("user:{nour}")
    )));
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn confusable_usernames_are_rejected() {
    let h = Harness::with_config(|c| c.auth.rate_limits.signup_per_ip.max = 100).await;
    let anon = h.anon();
    ops::signup(&anon, &signup_body("ahmed"))
        .await
        .expect("first");
    let taken = plain("username_taken", "Username not available", 409, None);
    // Cyrillic "а" (U+0430) in place of the Latin "a".
    assert_problem(
        ops::signup(&anon, &signup_body("\u{0430}hmed")).await,
        &taken,
    );
    assert_problem(ops::signup(&anon, &signup_body("AHMED")).await, &taken);
    assert_problem(ops::signup(&anon, &signup_body("ＡＨＭＥＤ")).await, &taken);
    ops::signup(&anon, &signup_body("ace")).await.expect("ace");
    // All-Cyrillic "асе".
    assert_problem(
        ops::signup(&anon, &signup_body("\u{0430}\u{0441}\u{0435}")).await,
        &taken,
    );
    ops::signup(&anon, &signup_body("ahmad"))
        .await
        .expect("a different name");
    assert_problem(
        ops::signup(&anon, &signup_body("bad name")).await,
        &invalid_field(
            "invalid_username",
            "/username",
            "usernames may contain only letters, digits, '.', '_' and '-'",
        ),
    );
    let err = ops::signup(
        &anon,
        &types::SignupRequest {
            username: "valid-name".into(),
            password: "short".into(),
            display_name: "V".into(),
        },
    )
    .await
    .expect_err("short password");
    assert_eq!(problem(&err).errors[0].code, "invalid_password");
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn devices_are_listed_renamed_and_removed_within_the_callers_scope() {
    let h = Harness::new().await;
    let (_, admin) = h.admin().await;
    let lina_id = h
        .create_user("lina", "lina-password-1", UserRole::Member)
        .await;
    let first = h.login("lina", "lina-password-1").await;
    h.clock.advance(Duration::seconds(5));
    let second = h.login("lina", "lina-password-1").await;
    let lina = h.with_token(&second.access_token);

    let devices = ops::list_devices(&lina).await.expect("devices");
    assert_eq!(
        devices,
        vec![
            types::Device {
                created: h.now(),
                current: true,
                id: second.device_id,
                last_seen: h.now(),
                name: "test-device".into(),
                platform: types::DevicePlatform::Linux,
                reminders_enabled: true,
            },
            types::Device {
                created: h.now() - Duration::seconds(5),
                current: false,
                id: first.device_id,
                last_seen: h.now() - Duration::seconds(5),
                name: "test-device".into(),
                platform: types::DevicePlatform::Linux,
                reminders_enabled: true,
            },
        ]
    );
    let renamed = ops::update_device(
        &lina,
        first.device_id,
        &types::UpdateDevice {
            name: Some("Pixel".into()),
            reminders_enabled: Some(false),
        },
    )
    .await
    .expect("patch");
    assert_eq!(renamed.name, "Pixel");
    assert!(!renamed.reminders_enabled);
    assert!(!renamed.current);
    // Stored in `devices.reminders_enabled`, not in the user's settings.
    let mut tx = h.db.begin(lina_id).await.expect("scope");
    let rows: Vec<(String, bool)> =
        sqlx::query_as("SELECT name, reminders_enabled FROM devices ORDER BY created")
            .fetch_all(tx.conn())
            .await
            .expect("devices");
    let settings: Vec<String> = sqlx::query_scalar("SELECT key FROM settings")
        .fetch_all(tx.conn())
        .await
        .expect("settings");
    tx.commit().await.expect("commit");
    assert_eq!(
        rows,
        vec![
            ("Pixel".to_owned(), false),
            ("test-device".to_owned(), true)
        ]
    );
    assert_eq!(settings, Vec::<String>::new());
    let listed = ops::list_devices(&lina).await.expect("devices");
    assert_eq!(
        listed
            .iter()
            .map(|d| (d.id, d.reminders_enabled))
            .collect::<Vec<_>>(),
        vec![(second.device_id, true), (first.device_id, false)]
    );
    assert_problem(
        ops::update_device(
            &lina,
            first.device_id,
            &types::UpdateDevice {
                name: Some(String::new()),
                reminders_enabled: None,
            },
        )
        .await,
        &invalid_field(
            "invalid_device_name",
            "/device_name",
            "device names have 1 to 100 characters and no control characters",
        ),
    );

    // Another user's device ID is indistinguishable from a missing one — even for an admin.
    let not_found = plain("not_found", "Not found", 404, None);
    assert_problem(
        ops::delete_device(&admin, first.device_id).await,
        &not_found,
    );
    assert_problem(
        ops::update_device(&admin, first.device_id, &types::UpdateDevice::default()).await,
        &not_found,
    );

    // Removing a device ends its session immediately.
    ops::delete_device(&lina, first.device_id)
        .await
        .expect("delete");
    assert_problem(
        ops::get_me(&h.with_token(&first.access_token)).await,
        &unauthorized("session revoked"),
    );
    assert_problem(
        ops::refresh(&h.anon(), &refresh_body(&first.refresh_token)).await,
        &unauthorized("invalid or expired refresh token"),
    );
    assert_eq!(ops::list_devices(&lina).await.expect("devices").len(), 1);
    assert_problem(ops::delete_device(&lina, first.device_id).await, &not_found);
    h.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn settings_round_trip_through_patch_me() {
    let h = Harness::new().await;
    h.create_user("sara", "sara-password-1", UserRole::Member)
        .await;
    let client = h.with_token(&h.login("sara", "sara-password-1").await.access_token);
    let me = ops::update_me(
        &client,
        &types::UpdateMe {
            display_name: Some("  سارة  ".into()),
            timezone: Some("Africa/Cairo".into()),
            ui_language: Some(types::UiLanguage::Ar),
            preferences: Some([("theme".to_owned(), "dark".to_owned())].into()),
            ..Default::default()
        },
    )
    .await
    .expect("patch");
    assert_eq!(me.display_name, "سارة");
    assert_eq!(me.timezone, "Africa/Cairo");
    assert_eq!(me.ui_language, types::UiLanguage::Ar);
    assert_eq!(
        me.preferences,
        [("theme".to_owned(), "dark".to_owned())].into()
    );
    assert_eq!(ops::get_me(&client).await.expect("me"), me);
    assert_problem(
        ops::update_me(
            &client,
            &types::UpdateMe {
                timezone: Some("Mars/Base".into()),
                ..Default::default()
            },
        )
        .await,
        &invalid_field("invalid_timezone", "/timezone", "not an IANA timezone"),
    );
    let too_many: std::collections::HashMap<String, String> =
        (0..65).map(|i| (format!("k{i}"), "v".to_owned())).collect();
    assert_problem(
        ops::update_me(
            &client,
            &types::UpdateMe {
                preferences: Some(too_many),
                ..Default::default()
            },
        )
        .await,
        &invalid_field(
            "invalid_preferences",
            "/preferences",
            "at most 64 preferences; keys of 1 to 64 bytes, values up to 1024 bytes",
        ),
    );
    h.finish().await;
}

/// Makes every insert into `strata.<table>` fail (installed as the superuser, test database
/// only) until [`heal`] runs.
async fn break_inserts(h: &Harness, table: &str) {
    for stmt in [
        "CREATE OR REPLACE FUNCTION strata.test_injected_failure() RETURNS trigger \
         LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected failure'; END $$"
            .to_owned(),
        format!(
            "CREATE TRIGGER test_injected_failure BEFORE INSERT ON strata.{table} \
             FOR EACH ROW EXECUTE FUNCTION strata.test_injected_failure()"
        ),
    ] {
        sqlx::query(sqlx::AssertSqlSafe(stmt))
            .execute(&h.db.superuser)
            .await
            .expect("inject failure");
    }
}

async fn heal(h: &Harness, table: &str) {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER test_injected_failure ON strata.{table}"
    )))
    .execute(&h.db.superuser)
    .await
    .expect("remove failure");
}

/// `(devices, sessions, refresh tokens)` rows, and the refresh tokens not yet spent.
async fn account_rows(h: &Harness) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM devices), (SELECT count(*) FROM sessions), \
                (SELECT count(*) FROM refresh_tokens), \
                (SELECT count(*) FROM refresh_tokens WHERE used_at IS NULL)",
    )
    .fetch_one(&h.db.accounts)
    .await
    .expect("counts")
}

#[tokio::test(flavor = "multi_thread")]
async fn login_and_refresh_writes_are_one_transaction() {
    let h = Harness::new().await;
    h.create_user("omar", "omar-password-1", UserRole::Member)
        .await;
    let internal = plain("internal", "Internal server error", 500, None);

    // The refresh-token insert (the last write of a login) fails: no device or session is
    // left behind.
    break_inserts(&h, "refresh_tokens").await;
    assert_problem(h.try_login("omar", "omar-password-1").await, &internal);
    assert_eq!(account_rows(&h).await, (0, 0, 0, 0));
    // The session insert (the middle write) fails: no device either.
    heal(&h, "refresh_tokens").await;
    break_inserts(&h, "sessions").await;
    assert_problem(h.try_login("omar", "omar-password-1").await, &internal);
    assert_eq!(account_rows(&h).await, (0, 0, 0, 0));
    heal(&h, "sessions").await;
    let session = h.login("omar", "omar-password-1").await;
    assert_eq!(account_rows(&h).await, (1, 1, 1, 1));

    // A rotation whose new-token insert fails spends nothing and touches nothing: the old
    // token still works afterwards (and is not treated as reuse).
    h.clock.advance(Duration::minutes(20));
    break_inserts(&h, "refresh_tokens").await;
    assert_problem(
        ops::refresh(&h.anon(), &refresh_body(&session.refresh_token)).await,
        &internal,
    );
    assert_eq!(account_rows(&h).await, (1, 1, 1, 1));
    let last_seen: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar("SELECT last_seen FROM devices")
            .fetch_one(&h.db.accounts)
            .await
            .expect("device");
    assert_eq!(last_seen, h.now() - Duration::minutes(20));
    heal(&h, "refresh_tokens").await;
    let rotated = ops::refresh(&h.anon(), &refresh_body(&session.refresh_token))
        .await
        .expect("old token still valid");
    assert_eq!(
        (rotated.session_id, rotated.device_id),
        (session.session_id, session.device_id)
    );
    assert_eq!(account_rows(&h).await, (1, 1, 2, 1));
    let me = ops::get_me(&h.with_token(&rotated.access_token))
        .await
        .expect("me");
    assert_eq!(me.username, "omar");
    h.finish().await;
}

trait AuditExt {
    async fn state_audit(&self) -> Vec<(Option<strata_common::UserId>, String, String)>;
}

impl AuditExt for Harness {
    async fn state_audit(&self) -> Vec<(Option<strata_common::UserId>, String, String)> {
        self.db
            .accounts_db
            .list_audit(1000)
            .await
            .expect("audit")
            .into_iter()
            .map(|e| (e.actor_id, e.action, e.target))
            .collect()
    }
}
