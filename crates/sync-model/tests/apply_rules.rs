//! Pure op application rules.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers outside #[test] fns

use std::collections::BTreeMap;
use std::str::FromStr;

use chrono::NaiveDate;
use domain::{CustodyEventType, DocumentStatus, Priority};
use pretty_assertions::assert_eq;
use sync_model::Op;
use sync_model::apply::*;
use sync_model::ops::{
    DocumentCustody, EntityPatch, NoteRef, TaskCancel, TaskComplete, TaskRef, TaskUpdate,
};
use ulid::Ulid;
use vault_format::custody::CustodyState;
use vault_format::{Document, RelationKey};

fn d(s: &str) -> NaiveDate {
    NaiveDate::from_str(s).unwrap()
}

fn rk(s: &str) -> RelationKey {
    s.parse().unwrap()
}

#[test]
fn relation_ops_edit_the_source_frontmatter() {
    let mut doc = Document::parse(
        "---\nid: 01J8ZK3M4X7Q0000000000000A\nrelated: [\"[[Churn notes]]\"]\n---\nbody\n",
    );
    let fm = doc.frontmatter_mut();
    assert_eq!(
        relation_add(fm, rk("contradicts"), "Discount policy"),
        Ok(true)
    );
    assert_eq!(
        relation_add(fm, rk("contradicts"), "Discount policy"),
        Ok(false)
    );
    assert_eq!(
        relation_retype(fm, rk("related"), rk("supports"), "Churn notes"),
        Ok(())
    );
    assert_eq!(
        relation_retype(fm, rk("related"), rk("supports"), "Churn notes"),
        Err(ApplyError::RelationMissing {
            relation: rk("related"),
            target: "Churn notes".into()
        })
    );
    assert_eq!(relation_remove(fm, rk("follows-up"), "Nothing"), Ok(false));
    assert_eq!(
        doc.render(),
        "---\nid: 01J8ZK3M4X7Q0000000000000A\nsupports: [\"[[Churn notes]]\"]\ncontradicts: [\"[[Discount policy]]\"]\n---\nbody\n"
    );
    assert_eq!(
        relation_remove(doc.frontmatter_mut(), rk("supports"), "Churn notes"),
        Ok(true)
    );
    assert_eq!(
        relation_remove(doc.frontmatter_mut(), rk("contradicts"), "Discount policy"),
        Ok(true)
    );
    assert_eq!(
        doc.render(),
        "---\nid: 01J8ZK3M4X7Q0000000000000A\n---\nbody\n"
    );
}

#[test]
fn entity_patch_sets_fields_and_aliases() {
    let mut doc = Document::parse(
        "---\nid: 01J\nkind: person\naliases: [أحمد سمير, Ahmed S.]\nrole: Ops\nphone: \"\"\n---\n## Notes\n",
    );
    let patch = EntityPatch {
        id: Ulid::from(1_u128),
        set: BTreeMap::from([
            ("role".to_owned(), "Operations manager".to_owned()),
            ("nickname".to_owned(), "Samir".to_owned()),
        ]),
        unset: vec!["phone".into()],
        add_aliases: vec!["A. Samir".into(), "أحمد سمير".into()],
        remove_aliases: vec!["Ahmed S.".into()],
    };
    assert_eq!(entity_patch(doc.frontmatter_mut(), &patch), Ok(()));
    assert_eq!(
        doc.render(),
        "---\nid: 01J\nkind: person\naliases: [أحمد سمير, A. Samir]\nrole: Operations manager\nnickname: Samir\n---\n## Notes\n"
    );
    for (key, err) in [
        ("id", ApplyError::ImmutableKey("id".into())),
        ("kind", ApplyError::ImmutableKey("kind".into())),
        ("location", ApplyError::NotPatchable("location".into())),
        ("aliases", ApplyError::NotPatchable("aliases".into())),
        ("works-at", ApplyError::NotPatchable("works-at".into())),
    ] {
        let before = doc.render();
        let bad = EntityPatch {
            id: Ulid::from(1_u128),
            set: BTreeMap::from([("role".to_owned(), "x".to_owned())]),
            unset: vec![key.into()],
            ..EntityPatch::default()
        };
        assert_eq!(entity_patch(doc.frontmatter_mut(), &bad), Err(err));
        assert_eq!(doc.render(), before, "nothing changes on error");
    }
}

fn custody_op(
    event: CustodyEventType,
    at: &str,
    place: Option<u128>,
    person: Option<u128>,
) -> DocumentCustody {
    DocumentCustody {
        document_id: Ulid::from(1_u128),
        event,
        at: d(at),
        place_id: place.map(Ulid::from),
        person_id: person.map(Ulid::from),
        counterparty_id: None,
    }
}

fn links(id: Ulid) -> Option<String> {
    match u128::from(id) {
        2 => Some("[[Safe — Nasr City office]]".into()),
        3 => Some("[[Shady]]".into()),
        _ => None,
    }
}

#[test]
fn spec_custody_example_updates_frontmatter_and_section() {
    let mut doc = Document::parse(
        "---\nid: 01J8ZK3M4X7Q0000000000000A\nkind: document\nlocation: \"\"\nstatus: stored\n---\n## Summary\nThe contract.\n\n## Custody\n- 2026-09-01 — stored-at [[Nasr City office]] — [[Capture 2026-09-01#^a1]]\n\n## Notes\nmine\n",
    );
    // "Watanya's contract is at the Nasr City office in the safe, last with Shady".
    let event = custody_event(
        &custody_op(CustodyEventType::StoredAt, "2026-09-20", Some(2), Some(3)),
        links,
        vec!["[[Capture 2026-09-20#^c1d2]]".into()],
    )
    .unwrap();
    let state = record_custody(&mut doc, event).unwrap();
    assert_eq!(
        state,
        CustodyState {
            location: Some("[[Safe — Nasr City office]]".into()),
            holder: None,
            last_holder: Some("[[Shady]]".into()),
            status: DocumentStatus::Stored,
        }
    );
    assert_eq!(
        doc.render(),
        "---\nid: 01J8ZK3M4X7Q0000000000000A\nkind: document\nlocation: \"[[Safe — Nasr City office]]\"\nholder: \"\"\nlast-holder: \"[[Shady]]\"\nstatus: stored\n---\n## Summary\nThe contract.\n\n## Custody\n- 2026-09-20 — stored-at [[Safe — Nasr City office]] by [[Shady]] — [[Capture 2026-09-20#^c1d2]]\n- 2026-09-01 — stored-at [[Nasr City office]] — [[Capture 2026-09-01#^a1]]\n\n## Notes\nmine\n"
    );
}

#[test]
fn custody_section_is_created_before_notes() {
    let mut doc = Document::parse("---\nkind: document\n---\n## Summary\nx\n\n## Notes\nmine\n");
    let event = custody_event(
        &custody_op(CustodyEventType::HandedTo, "2026-09-21", None, Some(3)),
        links,
        vec!["[[C]]".into()],
    )
    .unwrap();
    assert_eq!(
        record_custody(&mut doc, event).unwrap().status,
        DocumentStatus::CheckedOut
    );
    assert_eq!(
        doc.render(),
        "---\nkind: document\nlocation: \"\"\nholder: \"[[Shady]]\"\nlast-holder: \"[[Shady]]\"\nstatus: checked-out\n---\n## Summary\nx\n\n## Custody\n- 2026-09-21 — handed-to [[Shady]] — [[C]]\n\n## Notes\nmine\n"
    );
}

#[test]
fn same_day_event_is_the_newest() {
    let mut doc = Document::parse("## Custody\n- 2026-09-21 — handed-to [[Shady]] — [[C]]\n");
    let event = custody_event(
        &custody_op(CustodyEventType::ReturnedBy, "2026-09-21", Some(2), Some(3)),
        links,
        vec!["[[D]]".into()],
    )
    .unwrap();
    let state = record_custody(&mut doc, event).unwrap();
    assert_eq!((state.holder, state.status), (None, DocumentStatus::Stored));
    assert_eq!(
        doc.body(),
        "## Custody\n- 2026-09-21 — returned-by [[Shady]] to [[Safe — Nasr City office]] — [[D]]\n- 2026-09-21 — handed-to [[Shady]] — [[C]]\n"
    );
}

#[test]
fn custody_errors() {
    assert_eq!(
        custody_event(
            &custody_op(CustodyEventType::StoredAt, "2026-09-20", Some(9), None),
            links,
            vec!["[[C]]".into()]
        ),
        Err(ApplyError::UnknownEntity(Ulid::from(9_u128)))
    );
    assert!(matches!(
        custody_event(
            &custody_op(CustodyEventType::StoredAt, "2026-09-20", None, None),
            links,
            vec!["[[C]]".into()]
        ),
        Err(ApplyError::InvalidCustodyEvent(_))
    ));
    let mut doc = Document::parse("## Custody\n- garbage\n");
    let event = custody_event(
        &custody_op(CustodyEventType::Lost, "2026-09-20", None, None),
        links,
        vec!["[[C]]".into()],
    )
    .unwrap();
    assert_eq!(
        record_custody(&mut doc, event),
        Err(ApplyError::MalformedCustody(1))
    );
    assert_eq!(doc.body(), "## Custody\n- garbage\n");
}

const TASKS: &str = "# Tasks\n- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2\n- [ ] Call Shady 📅 2026-09-28 ^t-2\n";
const WATANYA: &str = "- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2\n";

#[test]
fn completing_a_recurring_task_writes_the_next_occurrence_above() {
    let op = Op::TaskComplete(TaskComplete {
        id: "t-01j9a2".into(),
        done: d("2026-10-01"),
        next_id: Some("t-01j9b0".into()),
    });
    assert_eq!(
        apply_task_op(TASKS, &op).unwrap(),
        "# Tasks\n- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-11-01 (@2026-11-01 09:00) [[Watanya]] ^t-01j9b0\n- [x] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01 ✅ 2026-10-01 (@2026-10-01 09:00) [[Watanya]] ^t-01j9a2\n- [ ] Call Shady 📅 2026-09-28 ^t-2\n"
    );
    let no_next = Op::TaskComplete(TaskComplete {
        id: "t-01j9a2".into(),
        done: d("2026-10-01"),
        next_id: None,
    });
    assert_eq!(
        apply_task_op(TASKS, &no_next),
        Err(ApplyError::MissingNextId("t-01j9a2".into()))
    );
}

#[test]
fn task_line_edits() {
    let with = |line: &str| format!("# Tasks\n{WATANYA}{line}");
    let cases: Vec<(Op, String)> = vec![
        (
            Op::TaskComplete(TaskComplete {
                id: "t-2".into(),
                done: d("2026-09-28"),
                next_id: None,
            }),
            with("- [x] Call Shady 📅 2026-09-28 ✅ 2026-09-28 ^t-2\n"),
        ),
        (
            Op::TaskCancel(TaskCancel {
                id: "t-2".into(),
                date: d("2026-09-28"),
            }),
            with("- [-] Call Shady 📅 2026-09-28 ❌ 2026-09-28 ^t-2\n"),
        ),
        (Op::TaskDelete(TaskRef { id: "t-2".into() }), with("")),
        (
            Op::TaskUpdate(TaskUpdate {
                id: "t-2".into(),
                text: Some("Call Shady about the contract".into()),
                due: Some(None),
                priority: Some(Some(Priority::High)),
                ..TaskUpdate::default()
            }),
            with("- [ ] Call Shady about the contract ⏫ ^t-2\n"),
        ),
    ];
    for (op, want) in cases {
        assert_eq!(apply_task_op(TASKS, &op), Ok(want), "{}", op.kind());
    }
    let done = with("- [x] Call Shady 📅 2026-09-28 ✅ 2026-09-28 ^t-2\n");
    assert_eq!(
        apply_task_op(&done, &Op::TaskReopen(TaskRef { id: "t-2".into() })),
        Ok(TASKS.to_owned())
    );
    assert_eq!(
        apply_task_op(TASKS, &Op::TaskReopen(TaskRef { id: "t-9".into() })),
        Err(ApplyError::TaskNotFound("t-9".into()))
    );
    assert_eq!(
        apply_task_op(
            TASKS,
            &Op::RelinkRequest(NoteRef {
                id: Ulid::from(1_u128)
            })
        ),
        Err(ApplyError::NotATaskOp("relink.request".into()))
    );
    // Line versions ignore the terminator and identify the exact line.
    let (span, _) = find_task(TASKS, "t-2").unwrap();
    assert_eq!(
        task_line_version(&TASKS[span.clone()]),
        task_line_version(&format!("{}\r\n", &TASKS[span]))
    );
}

#[test]
fn task_at_end_without_newline_and_crlf() {
    let body = "- [ ] a ^t-1\r\n- [ ] b ^t-2";
    let op = Op::TaskComplete(TaskComplete {
        id: "t-2".into(),
        done: d("2026-09-28"),
        next_id: None,
    });
    assert_eq!(
        apply_task_op(body, &op),
        Ok("- [ ] a ^t-1\r\n- [x] b ✅ 2026-09-28 ^t-2".to_owned())
    );
    assert_eq!(
        apply_task_op(body, &Op::TaskDelete(TaskRef { id: "t-1".into() })),
        Ok("- [ ] b ^t-2".to_owned())
    );
}
