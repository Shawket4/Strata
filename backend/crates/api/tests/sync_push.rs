//! `POST /sync/push` (PLAN §7.5 Sync, §12.4, §16.3–16.4, D19).
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod sync_harness;

use pretty_assertions::assert_eq;
use sync_harness::{H, header, version};
use sync_model::ops::{self as o, Op};
use sync_model::{OpResult, SyncOp};
use ulid::Ulid;

fn op(n: u128, base: Option<sync_model::Version>, op: Op) -> SyncOp {
    SyncOp::new(Ulid(0x0199_0000_0000_0000_0000_0000_0000_0000 + n), base, op)
}

#[tokio::test]
async fn smoke() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let id = Ulid(0x0199_1111_0000_0000_0000_0000_0000_0001);
    let (res, _) = h
        .push(
            &alice,
            vec![op(
                1,
                None,
                Op::NoteCreate(o::NoteCreate {
                    id,
                    path: "notes/A.md".into(),
                    content: "# A\n\nline one\n".into(),
                    force: false,
                }),
            )],
        )
        .await;
    let text = h.read(alice.id, "notes/A.md");
    assert_eq!(text, format!("{}# A\n\nline one\n", header(&id.to_string())));
    assert_eq!(
        res.results[0].result,
        OpResult::Applied {
            new_version: Some(version(&text)),
            merged: false
        }
    );
    h.finish().await;
}
