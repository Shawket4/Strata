//! `AccountsDb`: user lifecycle, invites, audit log, devices, sessions, refresh-token rotation.
#![allow(clippy::expect_used, clippy::float_cmp, clippy::too_many_lines)] // tests: expect with messages, exact asserts

use chrono::Duration;
use pretty_assertions::assert_eq;
use strata_common::{AuditId, Clock, DeviceId, InviteId, SessionId, UserId};
use strata_index::accounts::{
    AuditEntry, Invite, NewUser, RefreshOutcome, RefreshToken, Session, User,
};
use strata_index::repo::devices;
use strata_index::types::{Platform, PushProvider, RevokeReason, UserRole, UserStatus};
use strata_testkit::{TestDb, TestUser};

fn signup(db: &TestDb, name: &str) -> NewUser {
    NewUser {
        id: UserId::generate(db.ids.as_ref()),
        username: name.to_owned(),
        username_normalized: name.to_lowercase(),
        display_name: format!("{name} display"),
        password_hash: "$argon2id$hash".into(),
        role: UserRole::Member,
        status: UserStatus::Pending,
        approved_by: None,
    }
}

#[tokio::test]
async fn signup_approve_disable_enable_lifecycle() {
    let db = TestDb::new().await.expect("db");
    let admin = TestUser::new("root")
        .admin()
        .create(&db)
        .await
        .expect("admin");
    let t0 = db.clock.now();
    let new = signup(&db, "Sara");
    let created = db.accounts_db.create_user(&new, t0).await.expect("signup");
    assert_eq!(
        created,
        User {
            id: new.id,
            username: "Sara".into(),
            username_normalized: "sara".into(),
            display_name: "Sara display".into(),
            password_hash: "$argon2id$hash".into(),
            role: UserRole::Member,
            status: UserStatus::Pending,
            created: t0,
            updated: t0,
            approved_by: None,
            approved_at: None,
            rejected_at: None,
            disabled_at: None,
            deletion_requested_by: None,
            deletion_requested_at: None,
            deletion_at: None,
            export_downloaded_at: None,
        }
    );
    assert_eq!(db.accounts_db.count_pending().await.expect("count"), 1);
    // Duplicate normalised username.
    let mut dup = signup(&db, "SARA");
    dup.username_normalized = "sara".into();
    let err = db
        .accounts_db
        .create_user(&dup, t0)
        .await
        .expect_err("taken");
    assert!(err.is_unique_violation(), "{err}");
    assert_eq!(err.constraint(), Some("users_username_normalized_key"));

    let t1 = t0 + Duration::hours(1);
    let approved = db
        .accounts_db
        .approve_user(new.id, admin.id, t1)
        .await
        .expect("approve")
        .expect("pending");
    assert_eq!(
        (
            approved.status,
            approved.approved_by,
            approved.approved_at,
            approved.updated
        ),
        (UserStatus::Active, Some(admin.id), Some(t1), t1)
    );
    assert_eq!(
        db.accounts_db
            .approve_user(new.id, admin.id, t1)
            .await
            .expect("again"),
        None,
        "only pending can be approved"
    );
    assert_eq!(
        db.accounts_db
            .reject_user(new.id, t1)
            .await
            .expect("reject"),
        None
    );

    let disabled = db
        .accounts_db
        .disable_user(new.id, t1)
        .await
        .expect("disable")
        .expect("active");
    assert_eq!(
        (disabled.status, disabled.disabled_at),
        (UserStatus::Disabled, Some(t1))
    );
    let enabled = db
        .accounts_db
        .enable_user(new.id, t1)
        .await
        .expect("enable")
        .expect("disabled");
    assert_eq!(
        (enabled.status, enabled.disabled_at),
        (UserStatus::Active, None)
    );

    let listed: Vec<String> = db
        .accounts_db
        .list_users(Some(UserStatus::Active))
        .await
        .expect("list")
        .into_iter()
        .map(|u| u.username)
        .collect();
    assert_eq!(listed, vec!["root", "Sara"]);
    assert_eq!(db.accounts_db.list_users(None).await.expect("all").len(), 2);
    assert_eq!(
        db.accounts_db
            .user_by_username("sara")
            .await
            .expect("lookup")
            .map(|u| u.id),
        Some(new.id)
    );
    assert_eq!(
        db.accounts_db
            .user_by_username("nobody")
            .await
            .expect("lookup"),
        None
    );

    let reset = db
        .accounts_db
        .set_password_hash(new.id, "$argon2id$new", t1)
        .await
        .expect("pw")
        .expect("user");
    assert_eq!(reset.password_hash, "$argon2id$new");
    let promoted = db
        .accounts_db
        .set_role(new.id, UserRole::Admin, t1)
        .await
        .expect("role")
        .expect("user");
    assert_eq!(promoted.role, UserRole::Admin);
}

#[tokio::test]
async fn rejection_and_deletion_scheduling_follow_the_state_machine() {
    let db = TestDb::new().await.expect("db");
    let admin = TestUser::new("root")
        .admin()
        .create(&db)
        .await
        .expect("admin");
    let t0 = db.clock.now();
    let pending = db
        .accounts_db
        .create_user(&signup(&db, "p"), t0)
        .await
        .expect("p");
    let rejected = db
        .accounts_db
        .reject_user(pending.id, t0)
        .await
        .expect("reject")
        .expect("pending");
    assert_eq!(
        (rejected.status, rejected.rejected_at),
        (UserStatus::Rejected, Some(t0))
    );
    assert_eq!(
        db.accounts_db
            .schedule_deletion(pending.id, admin.id, t0, t0)
            .await
            .expect("sched"),
        None
    );

    let member = TestUser::new("m").create(&db).await.expect("m");
    let purge_at = t0 + Duration::days(14);
    let sched = db
        .accounts_db
        .schedule_deletion(member.id, admin.id, t0, purge_at)
        .await
        .expect("sched")
        .expect("active");
    assert_eq!(
        (
            sched.status,
            sched.deletion_at,
            sched.deletion_requested_by,
            sched.deletion_requested_at
        ),
        (
            UserStatus::DeletionPending,
            Some(purge_at),
            Some(admin.id),
            Some(t0)
        )
    );
    assert_eq!(
        db.accounts_db.users_due_for_purge(t0).await.expect("due"),
        vec![]
    );
    let downloaded = db
        .accounts_db
        .mark_export_downloaded(member.id, t0 + Duration::days(1))
        .await
        .expect("mark")
        .expect("pending");
    assert_eq!(
        downloaded.export_downloaded_at,
        Some(t0 + Duration::days(1))
    );

    let cancelled = db
        .accounts_db
        .cancel_deletion(member.id, t0)
        .await
        .expect("cancel")
        .expect("pending");
    assert_eq!(
        (
            cancelled.status,
            cancelled.deletion_at,
            cancelled.deletion_requested_by
        ),
        (UserStatus::Active, None, None)
    );

    db.accounts_db
        .schedule_deletion(member.id, admin.id, t0, purge_at)
        .await
        .expect("sched")
        .expect("active");
    let confirmed = db
        .accounts_db
        .confirm_deletion(member.id, t0 + Duration::days(2))
        .await
        .expect("confirm")
        .expect("pending");
    assert_eq!(confirmed.deletion_at, Some(t0 + Duration::days(2)));
    assert_eq!(
        db.accounts_db
            .users_due_for_purge(t0 + Duration::days(2))
            .await
            .expect("due"),
        vec![member.id]
    );
    assert!(
        !db.accounts_db
            .purge_user(admin.id)
            .await
            .expect("not pending")
    );
    assert!(db.accounts_db.purge_user(member.id).await.expect("purge"));
    assert_eq!(
        db.accounts_db.user_by_id(member.id).await.expect("lookup"),
        None
    );
}

#[tokio::test]
async fn invites_are_single_use_and_expire() {
    let db = TestDb::new().await.expect("db");
    let admin = TestUser::new("root")
        .admin()
        .create(&db)
        .await
        .expect("admin");
    let t0 = db.clock.now();
    let invite = Invite {
        id: InviteId::generate(db.ids.as_ref()),
        token_hash: vec![1, 2, 3],
        role: UserRole::Member,
        created_by: Some(admin.id),
        created: t0,
        expires: t0 + Duration::days(7),
        used_at: None,
        used_by: None,
    };
    db.accounts_db.create_invite(&invite).await.expect("create");
    let user = TestUser::new("new").create(&db).await.expect("user");
    assert_eq!(
        db.accounts_db
            .consume_invite(&[1, 2, 3], user.id, t0 + Duration::days(8))
            .await
            .expect("expired"),
        None
    );
    let consumed = db
        .accounts_db
        .consume_invite(&[1, 2, 3], user.id, t0)
        .await
        .expect("consume")
        .expect("valid");
    assert_eq!(
        consumed,
        Invite {
            used_at: Some(t0),
            used_by: Some(user.id),
            ..invite
        }
    );
    assert_eq!(
        db.accounts_db
            .consume_invite(&[1, 2, 3], user.id, t0)
            .await
            .expect("reuse"),
        None
    );
    assert_eq!(
        db.accounts_db
            .consume_invite(&[9], user.id, t0)
            .await
            .expect("unknown"),
        None
    );
}

#[tokio::test]
async fn audit_log_lists_newest_first() {
    let db = TestDb::new().await.expect("db");
    let admin = TestUser::new("root")
        .admin()
        .create(&db)
        .await
        .expect("admin");
    let t0 = db.clock.now();
    let e1 = AuditEntry {
        id: AuditId::generate(db.ids.as_ref()),
        actor_id: Some(admin.id),
        action: "user.approve".into(),
        target: "user:x".into(),
        at: t0,
    };
    let e2 = AuditEntry {
        id: AuditId::generate(db.ids.as_ref()),
        actor_id: None,
        action: "user.purge".into(),
        target: "user:y".into(),
        at: t0 + Duration::seconds(1),
    };
    db.accounts_db.append_audit(&e1).await.expect("e1");
    db.accounts_db.append_audit(&e2).await.expect("e2");
    assert_eq!(
        db.accounts_db.list_audit(10).await.expect("list"),
        vec![e2.clone(), e1]
    );
    assert_eq!(db.accounts_db.list_audit(1).await.expect("list"), vec![e2]);
    // The app role may append (purge job) but not read.
    let err = sqlx::query("SELECT * FROM audit_log")
        .execute(&db.app)
        .await
        .expect_err("no read");
    assert!(strata_index::IndexError::from(err).is_permission_denied());
}

async fn login(
    db: &TestDb,
    user: UserId,
    t0: chrono::DateTime<chrono::Utc>,
) -> (DeviceId, SessionId) {
    let device = DeviceId::generate(db.ids.as_ref());
    db.accounts_db
        .create_device(user, device, "Pixel", Platform::Android, t0)
        .await
        .expect("device");
    let session = SessionId::generate(db.ids.as_ref());
    db.accounts_db
        .create_session(&Session {
            user_id: user,
            id: session,
            device_id: device,
            created: t0,
            expires: t0 + Duration::days(30),
            revoked_at: None,
            revoked_reason: None,
            export_only: false,
        })
        .await
        .expect("session");
    (device, session)
}

#[tokio::test]
async fn devices_and_sessions_across_accounts_and_app_roles() {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let b = TestUser::new("bob").create(&db).await.expect("b").id;
    let t0 = db.clock.now();
    let (dev_a, ses_a) = login(&db, a, t0).await;
    let (_dev_b, ses_b) = login(&db, b, t0).await;

    // App side, scoped: sees only its own device, can set a push token and revoke (logout).
    let mut tx = db.begin(a).await.expect("tx");
    let listed = devices::list_devices(&mut tx).await.expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (
            listed[0].id,
            listed[0].name.as_str(),
            listed[0].push_provider
        ),
        (dev_a, "Pixel", PushProvider::None)
    );
    assert!(
        devices::set_push(&mut tx, dev_a, PushProvider::Fcm, Some("tok"), t0)
            .await
            .expect("push")
    );
    let d = devices::get_device(&mut tx, dev_a)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(
        (d.push_provider, d.push_token.as_deref(), d.push_updated),
        (PushProvider::Fcm, Some("tok"), Some(t0))
    );
    assert!(
        devices::touch_device(&mut tx, dev_a, t0 + Duration::minutes(5))
            .await
            .expect("touch")
    );
    assert_eq!(
        devices::list_sessions(&mut tx)
            .await
            .expect("sessions")
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        vec![ses_a]
    );
    assert!(
        !devices::revoke_session(&mut tx, ses_b, RevokeReason::Logout, t0)
            .await
            .expect("foreign"),
        "B's session is invisible"
    );
    assert!(
        devices::revoke_session(&mut tx, ses_a, RevokeReason::Logout, t0)
            .await
            .expect("logout")
    );
    tx.commit().await.expect("commit");

    // Accounts side: sees both users' sessions; revocation set lists live revoked sessions.
    let s = db
        .accounts_db
        .session_by_id(ses_a)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(
        (s.revoked_at, s.revoked_reason),
        (Some(t0), Some(RevokeReason::Logout))
    );
    assert_eq!(
        db.accounts_db
            .revoke_user_sessions(b, RevokeReason::UserDisabled, t0)
            .await
            .expect("revoke"),
        vec![ses_b]
    );
    assert_eq!(
        db.accounts_db
            .revoke_user_sessions(b, RevokeReason::UserDisabled, t0)
            .await
            .expect("again"),
        vec![]
    );
    let mut live = vec![ses_a, ses_b];
    live.sort();
    assert_eq!(
        db.accounts_db.live_revocations(t0).await.expect("set"),
        live
    );
    assert_eq!(
        db.accounts_db
            .live_revocations(t0 + Duration::days(31))
            .await
            .expect("expired"),
        vec![]
    );

    // Removing a device removes its sessions.
    let mut tx = db.begin(a).await.expect("tx");
    assert_eq!(
        devices::delete_device(&mut tx, dev_a)
            .await
            .expect("delete"),
        Some(vec![ses_a])
    );
    assert_eq!(
        devices::delete_device(&mut tx, dev_a).await.expect("again"),
        None
    );
    tx.commit().await.expect("commit");
    assert_eq!(
        db.accounts_db.session_by_id(ses_a).await.expect("get"),
        None
    );
}

#[tokio::test]
async fn refresh_tokens_rotate_once_and_reuse_revokes_the_session() {
    let db = TestDb::new().await.expect("db");
    let a = TestUser::new("alice").create(&db).await.expect("a").id;
    let t0 = db.clock.now();
    let (_device, session) = login(&db, a, t0).await;
    let token = RefreshToken {
        user_id: a,
        token_hash: vec![7; 32],
        session_id: session,
        issued: t0,
        expires: t0 + Duration::days(30),
        used_at: None,
    };
    db.accounts_db
        .insert_refresh_token(&token)
        .await
        .expect("insert");

    let t1 = t0 + Duration::minutes(15);
    assert_eq!(
        db.accounts_db
            .use_refresh_token(&[7; 32], t1)
            .await
            .expect("use"),
        RefreshOutcome::Fresh(RefreshToken {
            used_at: Some(t1),
            ..token.clone()
        })
    );
    let t2 = t1 + Duration::minutes(1);
    assert_eq!(
        db.accounts_db
            .use_refresh_token(&[7; 32], t2)
            .await
            .expect("reuse"),
        RefreshOutcome::Reused(RefreshToken {
            used_at: Some(t1),
            ..token.clone()
        })
    );
    let s = db
        .accounts_db
        .session_by_id(session)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(
        (s.revoked_at, s.revoked_reason),
        (Some(t2), Some(RevokeReason::RefreshReuse))
    );
    // Once the session is revoked, every token of it is invalid.
    assert_eq!(
        db.accounts_db
            .use_refresh_token(&[7; 32], t2)
            .await
            .expect("revoked"),
        RefreshOutcome::Invalid
    );
    assert_eq!(
        db.accounts_db
            .use_refresh_token(&[8; 32], t2)
            .await
            .expect("unknown"),
        RefreshOutcome::Invalid
    );

    // Expired tokens are invalid.
    let (_d2, s2) = login(&db, a, t0).await;
    db.accounts_db
        .insert_refresh_token(&RefreshToken {
            user_id: a,
            token_hash: vec![9; 32],
            session_id: s2,
            issued: t0,
            expires: t0 + Duration::days(1),
            used_at: None,
        })
        .await
        .expect("insert");
    assert_eq!(
        db.accounts_db
            .use_refresh_token(&[9; 32], t0 + Duration::days(2))
            .await
            .expect("expired"),
        RefreshOutcome::Invalid
    );
}
