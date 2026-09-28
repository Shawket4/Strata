//! Merging two entities (PLAN §6.7) at the vault level: the loser's relations the survivor
//! lacks are carried over (links that would point at either of the two are dropped, as are
//! duplicates), its `## Notes` move under a dated heading of the survivor's `## Notes` (the
//! section is created when missing), its name and aliases become aliases, links to it are
//! rewritten, and everything lands in one commit.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod common;

use common::World;
use domain::RelationType;
use pretty_assertions::assert_eq;
use strata_vault::VaultError;
use strata_vault::ops::notes::CreateNote;
use strata_vault::ops::relations::AiEdge;
use vault_format::RelationKey;
use vault_format::sidecar::NoteSidecar;

fn note(path: &str, content: &str) -> CreateNote {
    CreateNote {
        created: strata_common::clock::default_test_epoch(),
        path: path.to_owned(),
        content: content.to_owned(),
        id: None,
        force: true,
    }
}

/// The file without its `id`, `created` and `updated` lines (IDs and times are the
/// harness's).
fn strip_stamps(text: &str) -> String {
    text.lines()
        .filter(|l| {
            !l.starts_with("id: ") && !l.starts_with("created: ") && !l.starts_with("updated: ")
        })
        .fold(String::new(), |mut out, l| {
            out.push_str(l);
            out.push('\n');
            out
        })
}

#[tokio::test]
async fn the_losers_relations_and_notes_move_into_the_survivor() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let create = |path: &'static str, content: &'static str| {
        let (vault, s) = (w.vault.clone(), s);
        async move {
            vault
                .create_note(&s, note(path, content))
                .await
                .expect(path)
        }
    };
    create("companies/Acme.md", "---\nkind: company\n---\n").await;
    create("companies/Nile.md", "---\nkind: company\n---\n").await;
    let survivor = create(
        "people/Mona Adel.md",
        "---\nkind: person\naliases: [Mona]\ncompanies: [\"[[Acme]]\"]\n---\n## Summary\n\nLeads pricing.\n\n## Notes\n\nPrefers email.\n\n## Timeline\n\n- 2026-09-01 — joined\n",
    )
    .await;
    let loser = create(
        "people/Mona A.md",
        "---\nkind: person\naliases: [منى]\ncompanies: [\"[[Acme]]\", \"[[Nile]]\", \"[[Mona Adel]]\"]\n---\n## Notes\n\nLikes tea.\n",
    )
    .await;
    let meeting = create("notes/Meeting.md", "Met [[Mona A]] and [[Mona Adel]].\n").await;
    // An AI relation to the loser: its provenance (in the meeting's sidecar) follows the merge.
    w.vault
        .ai_add_relations(
            &s,
            "link".into(),
            meeting.id,
            vec![AiEdge {
                rel: RelationKey::Note(RelationType::Related),
                dst: loser.id,
                confidence: 0.8,
                reason: "same person".into(),
                model: "fake/model".into(),
            }],
        )
        .await
        .expect("ai edge")
        .expect("a commit");
    let commits = w.log(u).len();

    w.vault
        .merge_entities(&s, loser.id, survivor.id)
        .await
        .expect("merge");

    assert_eq!(w.log(u).len(), commits + 1, "one commit");
    assert!(!w.exists(u, "people/Mona A.md"));
    assert_eq!(
        strip_stamps(&w.read(u, "people/Mona Adel.md")),
        "---\nkind: person\naliases: [Mona, Mona A, منى]\ncompanies: [\"[[Acme]]\", \"[[Nile]]\"]\n---\n## Summary\n\nLeads pricing.\n\n## Notes\n\nPrefers email.\n\n### Merged from Mona A (2026-09-27)\n\nLikes tea.\n\n## Timeline\n\n- 2026-09-01 — joined\n"
    );
    assert_eq!(
        vault_format::Document::parse(&w.read(u, "notes/Meeting.md")).body(),
        "Met [[Mona Adel]] and [[Mona Adel]].\n"
    );
    assert_eq!(
        w.last_commit_paths(u),
        [
            format!(".meta/notes/{}.json", meeting.id),
            ".trash/people/Mona A.md".to_owned(),
            "notes/Meeting.md".to_owned(),
            "people/Mona A.md".to_owned(),
            "people/Mona Adel.md".to_owned(),
        ]
    );
    let sidecar = NoteSidecar::from_json(&w.read(u, &format!(".meta/notes/{}.json", meeting.id)))
        .expect("sidecar");
    assert_eq!(
        sidecar
            .relations
            .iter()
            .map(|r| (r.target_id, r.confidence))
            .collect::<Vec<_>>(),
        [(survivor.id.as_ulid(), Some(0.8))]
    );
    assert_eq!(
        strip_stamps(&w.read(u, "notes/Meeting.md")),
        "---\nrelated: [\"[[Mona Adel]]\"]\n---\nMet [[Mona Adel]] and [[Mona Adel]].\n"
    );
    w.finish().await;
}

#[tokio::test]
async fn a_survivor_without_notes_gets_the_section_and_bad_merges_are_refused() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let create = |path: &'static str, content: &'static str| {
        let (vault, s) = (w.vault.clone(), s);
        async move {
            vault
                .create_note(&s, note(path, content))
                .await
                .expect(path)
        }
    };
    let survivor = create("people/Samir.md", "---\nkind: person\n---\nDrives.").await;
    let loser = create(
        "people/Samir H.md",
        "---\nkind: person\n---\n## Notes\n\nOwns the van.\n",
    )
    .await;
    let acme = create("companies/Acme.md", "---\nkind: company\n---\n").await;

    assert!(matches!(
        w.vault.merge_entities(&s, loser.id, loser.id).await,
        Err(VaultError::Invalid(m)) if m == "an entity cannot be merged into itself"
    ));
    assert!(matches!(
        w.vault.merge_entities(&s, loser.id, acme.id).await,
        Err(VaultError::Invalid(m)) if m == "only entities of the same kind can be merged"
    ));

    w.vault
        .merge_entities(&s, loser.id, survivor.id)
        .await
        .expect("merge");
    assert_eq!(
        strip_stamps(&w.read(u, "people/Samir.md")),
        "---\nkind: person\naliases: [Samir H]\n---\nDrives.\n\n## Notes\n\n### Merged from Samir H (2026-09-27)\n\nOwns the van.\n"
    );
    w.finish().await;
}
