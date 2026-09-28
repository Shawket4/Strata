//! Non-note file writes (PLAN §6.5, §6.8: saved map layouts, the cluster file): each
//! precondition on the file's version, the no-op write that makes no commit, removal, and
//! the paths that must go through the note operations instead.
#![allow(clippy::expect_used)]

mod common;

use common::World;
use pretty_assertions::assert_eq;
use strata_vault::ops::files::{Expect, FileWrite, FileWritten};
use strata_vault::{Author, VaultError};

fn write(path: &str, content: Option<&str>, expect: Expect) -> FileWrite {
    FileWrite {
        path: path.to_owned(),
        content: content.map(|c| c.as_bytes().to_vec()),
        expect,
        author: Author::User,
        op: "save map".to_owned(),
        index: None,
    }
}

#[tokio::test]
async fn file_writes_check_the_expected_version_and_commit_once() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let canvas = "{\"nodes\":[],\"edges\":[]}";
    assert_eq!(
        format!("{:?}", write("maps/Plan.canvas", Some(canvas), Expect::Any)),
        "FileWrite { path: \"maps/Plan.canvas\", bytes: Some(23), expect: Any, author: User, op: \"save map\", index: false }"
    );

    let created = w
        .vault
        .write_file(&s, write("maps/Plan.canvas", Some(canvas), Expect::Absent))
        .await
        .expect("create");
    let version = created.version.clone().expect("version");
    assert!(created.commit.is_some());
    assert_eq!(w.log(u)[0], "user: save map maps/Plan.canvas");
    assert_eq!(w.read(u, "maps/Plan.canvas"), canvas);
    let commits = w.log(u).len();

    // It exists now, or has another version: conflicts carrying the current version.
    for expect in [Expect::Absent, Expect::Version("0".repeat(64))] {
        assert!(matches!(
            w.vault
                .write_file(&s, write("maps/Plan.canvas", Some("{}"), expect))
                .await,
            Err(VaultError::VersionConflict { current }) if current == version
        ));
    }
    // A version expected of a missing file.
    assert!(matches!(
        w.vault
            .write_file(
                &s,
                write(
                    "maps/Other.canvas",
                    Some("{}"),
                    Expect::Version(version.clone())
                )
            )
            .await,
        Err(VaultError::NotFound)
    ));
    // Notes have their own operations.
    assert!(matches!(
        w.vault
            .write_file(&s, write("notes/Plan.md", Some("x"), Expect::Any))
            .await,
        Err(VaultError::InvalidName(m)) if m == "notes are written through the note operations"
    ));
    // The same bytes again: nothing to commit.
    assert_eq!(
        w.vault
            .write_file(
                &s,
                write(
                    "maps/Plan.canvas",
                    Some(canvas),
                    Expect::Version(version.clone())
                )
            )
            .await
            .expect("same"),
        FileWritten {
            commit: None,
            version: Some(version.clone()),
        }
    );
    assert_eq!(w.log(u).len(), commits);

    // Removal.
    let removed = w
        .vault
        .write_file(
            &s,
            write("maps/Plan.canvas", None, Expect::Version(version)),
        )
        .await
        .expect("remove");
    assert_eq!(removed.version, None);
    assert!(removed.commit.is_some());
    assert!(!w.exists(u, "maps/Plan.canvas"));
    assert_eq!(w.log(u).len(), commits + 1);
    w.finish().await;
}
