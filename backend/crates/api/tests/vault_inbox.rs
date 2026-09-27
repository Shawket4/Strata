//! Capture, inbox, suggestions and relations over HTTP (PLAN §6.4, §6.5, §6.9, §7.5,
//! §16.3): capture is never refused, likely duplicates become a `duplicate` suggestion that
//! can be replied to, accepted (keep both) or rejected; user edges are added, retyped and
//! removed; removing an AI edge records a rejection the AI respects.
#![allow(
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::many_single_char_names
)]

mod vault_harness;

use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use strata_common::NoteId;
use vault_harness::{H, assert_problem, not_found};

fn note(path: &str, content: &str) -> types::CreateNoteRequest {
    types::CreateNoteRequest {
        content: content.to_owned(),
        force: None,
        id: None,
        path: path.to_owned(),
    }
}

fn edge(src: ulid::Ulid, dst: ulid::Ulid, rel: &str) -> types::RelationRef {
    types::RelationRef {
        dst_id: dst,
        src_id: src,
        type_: rel.to_owned(),
    }
}

#[tokio::test]
async fn capture_is_never_refused_and_duplicates_become_suggestions() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let text = "Call Watanya about the ETA invoice for October";
    let first = ops::capture(c, &types::CaptureRequest { text: text.into() })
        .await
        .expect("capture");
    assert_eq!(first.note.path, "inbox/2026-09-27-120000.md");
    assert_eq!(first.duplicates, vec![]);
    assert_eq!(first.suggestion_id, None);
    assert_eq!(
        h.read(alice.id, "inbox/2026-09-27-120000.md"),
        format!(
            "---\nid: {}\ncreated: 2026-09-27T12:00:00+00:00\n---\n{text}\n",
            first.note.id
        )
    );
    assert_eq!(
        h.log(alice.id)[0],
        "user: capture inbox/2026-09-27-120000.md"
    );

    // The same text again: saved anyway (same second → a free name), with a suggestion.
    let second = ops::capture(c, &types::CaptureRequest { text: text.into() })
        .await
        .expect("never refused");
    assert_eq!(second.note.path, "inbox/2026-09-27-120000 2.md");
    assert_eq!(second.duplicates.len(), 1);
    assert_eq!(second.duplicates[0].id, first.note.id);
    assert_eq!(second.duplicates[0].match_level, types::MatchLevel::Exact);
    let sid = second.suggestion_id.expect("suggestion");

    h.clock.advance(chrono::Duration::minutes(5));
    let inbox = ops::get_inbox(c).await.expect("inbox");
    assert_eq!(
        inbox
            .items
            .iter()
            .map(|i| (
                i.note.id,
                i.suggestions.iter().map(|s| s.id).collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>(),
        vec![(second.note.id, vec![sid]), (first.note.id, vec![])]
    );
    let s = &inbox.items[0].suggestions[0];
    assert_eq!(s.kind, "duplicate");
    assert_eq!(s.status, types::SuggestionStatus::Pending);
    assert_eq!(s.note_id, Some(second.note.id));
    assert_eq!(
        s.payload,
        types::SuggestionPayload::Duplicate {
            candidates: second.duplicates.clone()
        }
    );

    // Reply, then accept (keep both): never flagged against each other again.
    let replied = ops::reply_suggestion(
        c,
        sid,
        &types::ReplyRequest {
            body: "They differ: one is for November".into(),
        },
    )
    .await
    .expect("reply");
    assert_eq!(replied.replies.len(), 1);
    assert_eq!(replied.replies[0].body, "They differ: one is for November");
    assert_eq!(replied.status, types::SuggestionStatus::Pending);
    let accepted = ops::accept_suggestion(c, sid).await.expect("accept");
    assert_eq!(accepted.status, types::SuggestionStatus::Accepted);
    assert_eq!(accepted.decided_at, Some(h.now()));
    let again = ops::accept_suggestion(c, sid).await.expect_err("decided");
    let p = vault_harness::problem(&again);
    assert_eq!(
        (p.status, p.detail.as_deref()),
        (422, Some("the suggestion was already decided"))
    );
    let pending = ops::list_suggestions(c, Some(&types::SuggestionStatus::Pending))
        .await
        .expect("pending");
    assert_eq!(pending.items, vec![]);
    let sidecar = h.read(alice.id, &format!(".meta/notes/{}.json", second.note.id));
    assert_eq!(
        sidecar,
        format!(
            "{{\n  \"id\": \"{}\",\n  \"relations\": [],\n  \"rejected\": [],\n  \"keep_both\": [\n    {{\n      \"other_id\": \"{}\",\n      \"at\": \"2026-09-27T12:05:00Z\"\n    }}\n  ]\n}}\n",
            second.note.id, first.note.id
        )
    );
    let scope = h.db.scope(alice.id);
    let mut tx = h.db.begin(alice.id).await.expect("tx");
    assert!(
        strata_index::repo::dedupe::is_keep_both(
            &mut tx,
            "capture",
            &first.note.id.to_string(),
            &second.note.id.to_string()
        )
        .await
        .expect("query")
    );
    tx.commit().await.expect("commit");

    // A third capture is flagged against both; rejecting keeps the notes.
    let third = ops::capture(c, &types::CaptureRequest { text: text.into() })
        .await
        .expect("never refused");
    let mut flagged: Vec<_> = third.duplicates.iter().map(|d| d.id).collect();
    flagged.sort();
    let mut want = vec![first.note.id, second.note.id];
    want.sort();
    assert_eq!(flagged, want);
    let rejected = ops::reject_suggestion(c, third.suggestion_id.expect("sid"))
        .await
        .expect("reject");
    assert_eq!(rejected.status, types::SuggestionStatus::Rejected);
    assert_eq!(
        ops::get_note(c, third.note.id).await.expect("kept").trashed,
        false
    );
    let accepted = ops::list_suggestions(c, Some(&types::SuggestionStatus::Accepted))
        .await
        .expect("accepted");
    assert_eq!(
        accepted.items.iter().map(|s| s.id).collect::<Vec<_>>(),
        vec![sid]
    );
    let rejected = ops::list_suggestions(c, Some(&types::SuggestionStatus::Rejected))
        .await
        .expect("rejected");
    assert_eq!(rejected.items.len(), 1);
    assert_eq!(
        ops::list_suggestions(c, None).await.expect("pending").items,
        vec![]
    );
    assert_problem(
        ops::accept_suggestion(c, ulid::Ulid::new()).await,
        &not_found(),
    );
    let _ = scope;
    h.finish().await;
}

#[tokio::test]
async fn relations_add_retype_remove_and_ai_rejections_stick() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let a = ops::create_note(c, &note("notes/Plan.md", "The plan.\n"))
        .await
        .expect("a");
    let b = ops::create_note(c, &note("notes/Budget.md", "The budget.\n"))
        .await
        .expect("b");
    let x = ops::create_note(c, &note("notes/Risks.md", "Risks.\n"))
        .await
        .expect("x");

    let added = ops::add_relation(c, &edge(a.id, b.id, "supports"))
        .await
        .expect("add");
    assert_eq!(
        added,
        types::RelationResult {
            changed: true,
            dst_id: b.id,
            src_id: a.id,
            type_: "supports".into()
        }
    );
    assert_eq!(h.log(alice.id)[0], "user: relation add notes/Plan.md");
    let plan = ops::get_note(c, a.id).await.expect("plan");
    assert_eq!(
        plan.content,
        format!(
            "---\nid: {}\ncreated: 2026-09-27T12:00:00+00:00\nupdated: 2026-09-27T12:00:00+00:00\nsupports: [\"[[Budget]]\"]\n---\nThe plan.\n",
            a.id
        )
    );
    let again = ops::add_relation(c, &edge(a.id, b.id, "supports"))
        .await
        .expect("no-op");
    assert!(!again.changed);
    assert_eq!(h.log(alice.id).len(), 5, "a no-op add writes nothing");
    let backlinks = ops::get_backlinks(c, b.id).await.expect("backlinks");
    assert_eq!(backlinks.groups.len(), 1);

    let retyped = ops::retype_relation(
        c,
        &types::RetypeRelation {
            dst_id: b.id,
            src_id: a.id,
            type_: "supports".into(),
            new_type: "part-of".into(),
        },
    )
    .await
    .expect("retype");
    assert_eq!(retyped.type_, "part-of");
    let plan = ops::get_note(c, a.id).await.expect("plan");
    assert!(
        plan.content.contains("part-of: [\"[[Budget]]\"]\n"),
        "{}",
        plan.content
    );
    assert!(!plan.content.contains("supports:"), "{}", plan.content);
    let removed = ops::remove_relation(c, &edge(a.id, b.id, "part-of"))
        .await
        .expect("remove");
    assert!(!removed.changed, "a user edge is not recorded as rejected");
    assert_problem(
        ops::remove_relation(c, &edge(a.id, b.id, "part-of")).await,
        &not_found(),
    );
    let bad = ops::add_relation(c, &edge(a.id, b.id, "likes"))
        .await
        .expect_err("unknown");
    assert_eq!(
        vault_harness::problem(&bad).errors[0].code,
        "unknown_relation_type"
    );

    // An AI edge with provenance, removed by the user → rejected, never re-added.
    let scope = h.db.scope(alice.id);
    let ai = |dst: ulid::Ulid| strata_vault::ops::relations::AiEdge {
        rel: "related".parse().expect("rel"),
        dst: NoteId::from_ulid(dst),
        confidence: 0.83,
        reason: "same project".into(),
        model: "local/test".into(),
    };
    let commit = h
        .vault
        .ai_add_relations(
            &scope,
            "link".into(),
            NoteId::from_ulid(a.id),
            vec![ai(x.id)],
        )
        .await
        .expect("ai")
        .expect("changed");
    assert_eq!(h.log(alice.id)[0], "ai: link notes/Plan.md");
    let removed = ops::remove_relation(c, &edge(a.id, x.id, "related"))
        .await
        .expect("remove");
    assert!(removed.changed, "an AI edge is recorded as rejected");
    let sidecar = h.read(alice.id, &format!(".meta/notes/{}.json", a.id));
    assert!(
        sidecar.contains(&format!("\"target_id\": \"{}\"", x.id)),
        "{sidecar}"
    );
    assert!(sidecar.contains("\"rejected\": [\n"), "{sidecar}");
    let mut tx = h.db.begin(alice.id).await.expect("tx");
    let blocked = strata_index::repo::graph::is_rejected(
        &mut tx,
        NoteId::from_ulid(a.id),
        NoteId::from_ulid(x.id),
        "related",
    )
    .await
    .expect("query");
    tx.commit().await.expect("commit");
    assert!(blocked);
    let readded = h
        .vault
        .ai_add_relations(
            &scope,
            "link".into(),
            NoteId::from_ulid(a.id),
            vec![ai(x.id)],
        )
        .await
        .expect("ai");
    assert_eq!(readded, None, "a rejected edge is never re-added");
    // The whole AI commit is revertible.
    let _ = commit;
    h.finish().await;
}
