//! Reverting a whole commit whose files changed again afterwards (PLAN §7.2 history): the
//! note's frontmatter keys the commit set are taken back (a key it added is removed) while
//! later keys stay, and the sidecar is reverted list by list, so the relation provenance the
//! commit added goes and the provenance added later stays.
#![allow(clippy::expect_used)]

mod common;

use common::World;
use domain::RelationType;
use pretty_assertions::assert_eq;
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

fn edge(rel: RelationType, dst: strata_common::NoteId, reason: &str) -> AiEdge {
    AiEdge {
        rel: RelationKey::Note(rel),
        dst,
        confidence: 0.8,
        reason: reason.to_owned(),
        model: "fake/model".to_owned(),
    }
}

#[tokio::test]
async fn a_later_ai_edge_survives_the_revert_of_an_earlier_one() {
    let w = World::new().await;
    let (u, s) = w.user("alice").await;
    let pricing = w
        .vault
        .create_note(&s, note("notes/Pricing.md", "Q4.\n"))
        .await
        .expect("pricing");
    let churn = w
        .vault
        .create_note(&s, note("notes/Churn.md", "Churn.\n"))
        .await
        .expect("churn");
    let roadmap = w
        .vault
        .create_note(&s, note("notes/Roadmap.md", "Roadmap.\n"))
        .await
        .expect("roadmap");
    let budget = w
        .vault
        .create_note(&s, note("notes/Budget.md", "Budget.\n"))
        .await
        .expect("budget");
    // An earlier edge: the sidecar exists before the commit that gets reverted (a sidecar
    // the reverted commit created and a later commit changed would be a conflict).
    w.vault
        .ai_add_relations(
            &s,
            "link".into(),
            pricing.id,
            vec![edge(RelationType::PartOf, budget.id, "a budget line")],
        )
        .await
        .expect("earlier edge")
        .expect("a commit");
    let first = w
        .vault
        .ai_add_relations(
            &s,
            "link".into(),
            pricing.id,
            vec![edge(RelationType::Related, churn.id, "churn drives price")],
        )
        .await
        .expect("first edge")
        .expect("a commit");
    w.vault
        .ai_add_relations(
            &s,
            "link".into(),
            pricing.id,
            vec![edge(
                RelationType::Supports,
                roadmap.id,
                "the roadmap cites it",
            )],
        )
        .await
        .expect("second edge")
        .expect("a commit");

    let (commit, paths) = w
        .vault
        .revert_commit(&s, first.clone())
        .await
        .expect("revert");
    assert!(commit.is_some());
    let sidecar_path = format!(".meta/notes/{}.json", pricing.id);
    assert_eq!(paths, [sidecar_path.clone(), "notes/Pricing.md".to_owned()]);
    assert_eq!(w.log(u)[0], format!("user: revert commit {}", &first[..12]));
    let doc = vault_format::Document::parse(&w.read(u, "notes/Pricing.md"));
    let fm = doc.frontmatter().expect("frontmatter");
    assert_eq!(
        (
            fm.relation(RelationKey::Note(RelationType::PartOf)),
            fm.relation(RelationKey::Note(RelationType::Related)),
            fm.relation(RelationKey::Note(RelationType::Supports))
        ),
        (
            vec!["[[Budget]]".to_owned()],
            Vec::<String>::new(),
            vec!["[[Roadmap]]".to_owned()]
        )
    );
    let sidecar = NoteSidecar::from_json(&w.read(u, &sidecar_path)).expect("sidecar");
    let mut kept: Vec<_> = sidecar
        .relations
        .iter()
        .map(|r| (r.target_id, r.reason.clone()))
        .collect();
    kept.sort();
    let mut expected = vec![
        (budget.id.as_ulid(), Some("a budget line".to_owned())),
        (
            roadmap.id.as_ulid(),
            Some("the roadmap cites it".to_owned()),
        ),
    ];
    expected.sort();
    assert_eq!(kept, expected);
    w.finish().await;
}
