//! The local building blocks under the session (PLAN §12.2, §12.1): the per-account database
//! (file and in-memory, removed with its WAL companions), secrets kept out of `Debug`
//! output, and the watcher hub's lifecycle (closed receivers, failing builders, unwatch and
//! clear).

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use chrono::DateTime;
use pretty_assertions::assert_eq;
use rusqlite::Connection;
use strata_core::CoreError;
use strata_core::net::SessionTokens;
use strata_core::store::tokens::{self, StoredTokens};
use strata_core::store::{AccountDb, StorePaths, delete_db_files};
use strata_core::view::hub::Recorder;
use strata_core::view::model::{Connectivity, NotificationMode, SyncActivity};
use strata_core::view::{Topics, ViewCtx, ViewHub};
use ulid::Ulid;

const USER: &str = "01K5DSSE00000000000000AAAA";

fn stored() -> StoredTokens {
    StoredTokens {
        device_id: "dev-1".into(),
        session_id: "ses-1".into(),
        access_token: "SECRET-ACCESS".into(),
        access_expires_at: "2026-09-27T11:00:00Z".into(),
        refresh_token: "SECRET-REFRESH".into(),
        refresh_expires_at: "2026-10-27T10:00:00Z".into(),
        export_only: false,
        updated_at: "2026-09-27T10:00:00Z".into(),
    }
}

#[test]
fn account_databases_open_migrated_and_are_deleted_with_their_companions() {
    let user = Ulid::from_string(USER).expect("id");
    let mem = AccountDb::open_in_memory(user).expect("in memory");
    assert_eq!((mem.user_id(), mem.path().as_os_str().is_empty()), (user, true));
    tokens::put(mem.conn(), &stored()).expect("put");
    assert_eq!(tokens::get(mem.conn()), Ok(Some(stored())));
    assert_eq!(mem.delete(), Ok(()), "nothing on disk to delete");

    let dir = tempfile::tempdir().expect("dir");
    let paths = StorePaths::new(dir.path());
    let mut db = AccountDb::open(&paths, user).expect("file");
    assert_eq!(db.path(), paths.account(user));
    {
        let tx = db.conn_mut().transaction().expect("tx");
        tokens::put(&tx, &stored()).expect("put");
        tx.commit().expect("commit");
    }
    let wal = {
        let mut p = paths.account(user).into_os_string();
        p.push("-wal");
        std::path::PathBuf::from(p)
    };
    assert!(paths.account(user).exists() && wal.exists());
    // Reopening finds the data (migrations are idempotent).
    drop(db);
    let db = AccountDb::open(&paths, user).expect("reopen");
    assert_eq!(tokens::get(db.conn()), Ok(Some(stored())));
    db.delete().expect("delete");
    assert!(!paths.account(user).exists() && !wal.exists());
    // Deleting again (or a path that never existed) is fine; an empty path is a no-op.
    assert_eq!(delete_db_files(&paths.account(user)), Ok(()));
    assert_eq!(delete_db_files(std::path::Path::new("")), Ok(()));
    // A directory where the file should be is an error, not a silent success.
    std::fs::create_dir_all(paths.account(user)).expect("dir");
    assert!(matches!(
        delete_db_files(&paths.account(user)),
        Err(CoreError::Storage(_))
    ));
}

#[test]
fn tokens_never_appear_in_debug_output() {
    let stored = format!("{:?}", stored());
    assert!(!stored.contains("SECRET"), "{stored}");
    assert_eq!(
        stored,
        "StoredTokens { device_id: \"dev-1\", session_id: \"ses-1\", access_expires_at: \
         \"2026-09-27T11:00:00Z\", refresh_expires_at: \"2026-10-27T10:00:00Z\", \
         export_only: false, .. }"
    );
    let session = SessionTokens {
        user_id: USER.into(),
        device_id: "dev-1".into(),
        session_id: "ses-1".into(),
        access_token: "SECRET-ACCESS".into(),
        access_expires_at: DateTime::UNIX_EPOCH,
        refresh_token: "SECRET-REFRESH".into(),
        refresh_expires_at: DateTime::UNIX_EPOCH,
        export_only: true,
        password_change_required: false,
    };
    assert_eq!(
        format!("{session:?}"),
        format!("SessionTokens {{ user_id: \"{USER}\", device_id: \"dev-1\", export_only: true, .. }}")
    );
}

fn ctx() -> ViewCtx {
    ViewCtx {
        now: DateTime::UNIX_EPOCH,
        tz: chrono_tz::UTC,
        connectivity: Connectivity::Online,
        activity: SyncActivity::default(),
        notification_mode: NotificationMode::OsScheduled,
        lang: strata_core::format::labels::Lang::En,
    }
}

fn value(c: &Connection) -> strata_core::error::CoreResult<i64> {
    Ok(c.query_row("SELECT v FROM t", [], |r| r.get(0))?)
}

#[test]
fn the_hub_drops_gone_receivers_keeps_failing_builders_and_stops_on_request() {
    let conn = Connection::open_in_memory().expect("db");
    conn.execute_batch("CREATE TABLE t (v INTEGER); INSERT INTO t VALUES (1);")
        .expect("schema");
    let mut hub = ViewHub::new();
    assert!(hub.is_empty());

    // A receiver gone before the first value: nothing is registered.
    let gone = Recorder::<i64>::new();
    gone.close();
    assert_eq!(
        hub.watch(&conn, &ctx(), Topics::NOTES, |c, _| value(c), gone.clone()),
        Ok(0)
    );
    assert_eq!((hub.len(), gone.all()), (0, vec![]));

    let a = Recorder::new();
    let b = Recorder::new();
    let id_a = hub
        .watch(&conn, &ctx(), Topics::NOTES, |c, _| value(c), a.clone())
        .expect("a");
    let id_b = hub
        .watch(
            &conn,
            &ctx(),
            Topics::NOTES | Topics::TASKS,
            |c, _| {
                let v = value(c)?;
                if v == 2 {
                    return Err(CoreError::Internal("builder failed".into()));
                }
                Ok(v * 10)
            },
            b.clone(),
        )
        .expect("b");
    assert_eq!((id_a, id_b, hub.len()), (1, 2, 2));
    assert_eq!(format!("{hub:?}"), "ViewHub { watchers: 2, .. }");

    // No topic: nothing rebuilds.
    conn.execute("UPDATE t SET v = 2", []).expect("update");
    assert_eq!(hub.notify(&conn, &ctx(), Topics::NONE), Ok(()));
    assert_eq!((a.all(), b.all()), (vec![1], vec![10]));
    // A failing builder reports the error but stays registered; the others still update.
    assert_eq!(
        hub.notify(&conn, &ctx(), Topics::NOTES),
        Err(CoreError::Internal("builder failed".into()))
    );
    assert_eq!((a.all(), b.all(), hub.len()), (vec![1, 2], vec![10], 2));
    conn.execute("UPDATE t SET v = 3", []).expect("update");
    assert_eq!(hub.notify(&conn, &ctx(), Topics::TASKS), Ok(()));
    assert_eq!((a.take(), b.take()), (vec![1, 2], vec![10, 30]));

    hub.unwatch(id_b);
    assert_eq!(hub.len(), 1);
    hub.unwatch(99);
    assert_eq!(hub.len(), 1);
    hub.clear();
    assert!(hub.is_empty());
    conn.execute("UPDATE t SET v = 4", []).expect("update");
    assert_eq!(hub.notify(&conn, &ctx(), Topics::ALL), Ok(()));
    assert_eq!((a.all(), b.all()), (vec![], vec![]));
}
