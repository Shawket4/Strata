//! Migration `…010` (account columns) moves the interim encodings into columns:
//! `device.<id>.reminders_enabled` settings rows → `devices.reminders_enabled`, and the
//! `temporary:` password-hash prefix → `users.must_change_password`.
#![allow(clippy::expect_used, clippy::too_many_lines)] // tests: expect with messages

use pretty_assertions::assert_eq;
use strata_common::{DeviceId, UserId};
use strata_index::MIGRATOR;
use strata_testkit::TestDb;

const ACCOUNT_COLUMNS: i64 = 20_260_927_000_010;

async fn insert_user(db: &TestDb, name: &str, hash: &str) -> UserId {
    let id = UserId::generate(db.ids.as_ref());
    sqlx::query(
        "INSERT INTO users (id, username, username_normalized, display_name, password_hash, role, \
         status, created, updated, approved_at) \
         VALUES ($1, $2, $2, $2, $3, 'member', 'active', now(), now(), now())",
    )
    .bind(id)
    .bind(name)
    .bind(hash)
    .execute(&db.accounts)
    .await
    .expect("user");
    id
}

async fn insert_device(db: &TestDb, user: UserId) -> DeviceId {
    let id = DeviceId::generate(db.ids.as_ref());
    sqlx::query(
        "INSERT INTO devices (user_id, id, name, platform, created, last_seen) \
         VALUES ($1, $2, 'Phone', 'android', now(), now())",
    )
    .bind(user)
    .bind(id)
    .execute(&db.accounts)
    .await
    .expect("device");
    id
}

async fn put_setting(db: &TestDb, user: UserId, key: &str, value: &[u8]) {
    let mut tx = db.begin(user).await.expect("scope");
    sqlx::query(
        "INSERT INTO settings (user_id, key, value, updated) \
         VALUES (strata_current_user(), $1, $2, now())",
    )
    .bind(key)
    .bind(value)
    .execute(tx.conn())
    .await
    .expect("setting");
    tx.commit().await.expect("commit");
}

#[tokio::test]
async fn account_columns_migration_moves_settings_and_password_flags() {
    let db = TestDb::new_unmigrated().await.expect("db");
    let before = MIGRATOR
        .iter()
        .map(|m| m.version)
        .filter(|v| *v < ACCOUNT_COLUMNS)
        .max()
        .expect("earlier migrations");
    MIGRATOR.run_to(before, &db.owner).await.expect("up to 009");

    let a = insert_user(
        &db,
        "alice",
        "temporary:$argon2id$v=19$m=8,t=1,p=1$c2FsdA$aGFzaA",
    )
    .await;
    let b = insert_user(&db, "bob", "$argon2id$v=19$m=8,t=1,p=1$c2FsdA$Ym9i").await;
    let (a_off, a_on, a_bad, a_default) = (
        insert_device(&db, a).await,
        insert_device(&db, a).await,
        insert_device(&db, a).await,
        insert_device(&db, a).await,
    );
    let b_off = insert_device(&db, b).await;
    // MessagePack false (0xc2), true (0xc3), and a value the API could not decode.
    put_setting(
        &db,
        a,
        &format!("device.{a_off}.reminders_enabled"),
        &[0xc2],
    )
    .await;
    put_setting(&db, a, &format!("device.{a_on}.reminders_enabled"), &[0xc3]).await;
    put_setting(
        &db,
        a,
        &format!("device.{a_bad}.reminders_enabled"),
        &[0x01],
    )
    .await;
    put_setting(
        &db,
        a,
        "timezone",
        &[
            0xad, b'A', b'f', b'r', b'i', b'c', b'a', b'/', b'C', b'a', b'i', b'r', b'o',
        ],
    )
    .await;
    put_setting(
        &db,
        b,
        &format!("device.{b_off}.reminders_enabled"),
        &[0xc2],
    )
    .await;
    // A leftover row for a device that no longer exists is dropped.
    let gone = DeviceId::generate(db.ids.as_ref());
    put_setting(&db, b, &format!("device.{gone}.reminders_enabled"), &[0xc2]).await;

    MIGRATOR
        .run_to(ACCOUNT_COLUMNS, &db.owner)
        .await
        .expect("010");

    let users: Vec<(UserId, String, bool)> = sqlx::query_as(
        "SELECT id, password_hash, must_change_password FROM users ORDER BY username",
    )
    .fetch_all(&db.accounts)
    .await
    .expect("users");
    assert_eq!(
        users,
        vec![
            (
                a,
                "$argon2id$v=19$m=8,t=1,p=1$c2FsdA$aGFzaA".to_owned(),
                true
            ),
            (
                b,
                "$argon2id$v=19$m=8,t=1,p=1$c2FsdA$Ym9i".to_owned(),
                false
            ),
        ]
    );
    let mut devices: Vec<(DeviceId, bool)> =
        sqlx::query_as("SELECT id, reminders_enabled FROM devices")
            .fetch_all(&db.accounts)
            .await
            .expect("devices");
    devices.sort();
    let mut expected = vec![
        (a_off, false),
        (a_on, true),
        (a_bad, true),
        (a_default, true),
        (b_off, false),
    ];
    expected.sort();
    assert_eq!(devices, expected);
    for (user, keys) in [(a, vec!["timezone".to_owned()]), (b, vec![])] {
        let mut tx = db.begin(user).await.expect("scope");
        let left: Vec<String> = sqlx::query_scalar("SELECT key FROM settings ORDER BY key")
            .fetch_all(tx.conn())
            .await
            .expect("settings");
        tx.commit().await.expect("commit");
        assert_eq!(left, keys);
    }
    // The migration's scope did not outlive it.
    let scope: Option<String> =
        sqlx::query_scalar("SELECT NULLIF(current_setting('strata.user_id', true), '')")
            .fetch_one(&db.owner)
            .await
            .expect("scope");
    assert_eq!(scope, None);
}
