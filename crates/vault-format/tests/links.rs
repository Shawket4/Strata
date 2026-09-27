//! Link resolution and rename/move rewriting across a small vault, including frontmatter
//! relations, Arabic names, folder moves, collisions and canvas references.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pretty_assertions::assert_eq;
use vault_format::blocks::append_block_id;
use vault_format::canvas::Canvas;
use vault_format::rewrite::MoveSet;
use vault_format::{Document, PathIndex, RelationKey, Resolution, analyze};

const VAULT: &[&str] = &[
    "inbox/2026-09-27-101500.md",
    "notes/Pricing experiments.md",
    "notes/Churn notes.md",
    "notes/2026/Meeting.md",
    "notes/2025/Meeting.md",
    "people/أحمد سمير.md",
    "people/Shady.md",
    "companies/Acme Logistics.md",
    "concepts/Pricing.md",
    "attachments/2026/09/scan.pdf",
    "attachments/2026/09/photo.png",
];

fn r(p: &str) -> Resolution {
    Resolution::Resolved(p.to_owned())
}

#[test]
fn resolution_rules() {
    let idx = PathIndex::new(VAULT.iter().copied());
    let src = Some("inbox/2026-09-27-101500.md");
    assert_eq!(
        idx.resolve("Pricing experiments", src),
        r("notes/Pricing experiments.md")
    );
    assert_eq!(idx.resolve("pricing", src), r("concepts/Pricing.md"));
    assert_eq!(idx.resolve("أحمد سمير", src), r("people/أحمد سمير.md"));
    assert_eq!(
        idx.resolve("photo.png", src),
        r("attachments/2026/09/photo.png")
    );
    assert_eq!(
        idx.resolve("09/scan.pdf", src),
        r("attachments/2026/09/scan.pdf")
    );
    assert_eq!(
        idx.resolve("Meeting", src),
        Resolution::Ambiguous(vec![
            "notes/2025/Meeting.md".into(),
            "notes/2026/Meeting.md".into()
        ])
    );
    assert_eq!(
        idx.resolve("Meeting", Some("notes/2025/Other.md")),
        r("notes/2025/Meeting.md")
    );
    assert_eq!(idx.resolve("2026/Meeting", src), r("notes/2026/Meeting.md"));
    assert_eq!(idx.resolve("Nope", src), Resolution::Unresolved);
    assert_eq!(idx.resolve("", src), Resolution::CurrentNote);
    assert_eq!(
        idx.link_text_for("notes/2026/Meeting.md"),
        "notes/2026/Meeting"
    );
    assert_eq!(idx.link_text_for("people/أحمد سمير.md"), "أحمد سمير");
}

const NOTE: &str = "---\n\
id: 01J9A2B3C4D5E6F7G8H9J0K1M2\n\
related: [\"[[Pricing experiments]]\", \"[[Churn notes]]\"]\n\
people: [\"[[أحمد سمير]]\"]\n\
x-custom: \"[[Pricing experiments]]\"\n\
---\n\
See [[Pricing experiments|the tests]], [[Pricing experiments#Results]], \
![[Pricing experiments#^r1]] and [[أحمد سمير|أحمد]].\n\
```\n[[Pricing experiments]]\n```\n";

#[test]
fn rename_rewrites_body_and_frontmatter() {
    let before = PathIndex::new(VAULT.iter().copied());
    let after_paths: Vec<String> = VAULT
        .iter()
        .map(|p| match *p {
            "notes/Pricing experiments.md" => "notes/archive/Pricing tests 2026.md".to_owned(),
            "people/أحمد سمير.md" => "people/أحمد سمير عبد الله.md".to_owned(),
            p => p.to_owned(),
        })
        .collect();
    let after = PathIndex::new(after_paths);
    let moves = MoveSet::new(
        &before,
        &after,
        [
            (
                "notes/Pricing experiments.md".to_owned(),
                "notes/archive/Pricing tests 2026.md".to_owned(),
            ),
            (
                "people/أحمد سمير.md".to_owned(),
                "people/أحمد سمير عبد الله.md".to_owned(),
            ),
        ],
    );
    let mut doc = Document::parse(NOTE);
    let changed = doc.rewrite_links(&moves, "notes/Churn notes.md").unwrap();
    assert_eq!(changed, 6);
    assert_eq!(
        doc.render(),
        "---\n\
id: 01J9A2B3C4D5E6F7G8H9J0K1M2\n\
related: [\"[[Pricing tests 2026]]\", \"[[Churn notes]]\"]\n\
people: [\"[[أحمد سمير عبد الله]]\"]\n\
x-custom: \"[[Pricing experiments]]\"\n\
---\n\
See [[Pricing tests 2026|the tests]], [[Pricing tests 2026#Results]], \
![[Pricing tests 2026#^r1]] and [[أحمد سمير عبد الله|أحمد]].\n\
```\n[[Pricing experiments]]\n```\n"
    );
    assert_eq!(
        doc.frontmatter().unwrap().relation(RelationKey::Related),
        ["[[Pricing tests 2026]]", "[[Churn notes]]"]
    );
}

#[test]
fn untouched_note_is_byte_identical_after_rewrite_pass() {
    let before = PathIndex::new(VAULT.iter().copied());
    let moves = MoveSet::new(&before, &before, Vec::new());
    let mut doc = Document::parse(NOTE);
    assert_eq!(
        doc.rewrite_links(&moves, "notes/Churn notes.md").unwrap(),
        0
    );
    assert_eq!(doc.render(), NOTE);
}

#[test]
fn folder_move_rewrites_path_links_only_when_needed() {
    let before = PathIndex::new(VAULT.iter().copied());
    let after_paths: Vec<String> = VAULT
        .iter()
        .map(|p| p.replace("notes/2026/", "archive/2026/"))
        .collect();
    let after = PathIndex::new(after_paths);
    let moves = MoveSet::new(
        &before,
        &after,
        [(
            "notes/2026/Meeting.md".to_owned(),
            "archive/2026/Meeting.md".to_owned(),
        )],
    );
    let body = "[[notes/2026/Meeting]] [[2026/Meeting|m]] [[notes/2025/Meeting]] [[Meeting]]";
    let out = moves.rewrite_body(body, "people/Shady.md");
    // The ambiguous bare [[Meeting]] is left for the user; the others follow.
    assert_eq!(
        out.text,
        "[[archive/2026/Meeting]] [[2026/Meeting|m]] [[notes/2025/Meeting]] [[Meeting]]"
    );
    assert_eq!(out.changed, 1);
}

#[test]
fn canvas_references_follow_moves() {
    let mut c = Canvas::from_json(include_str!("fixtures/maps/Pricing.canvas")).unwrap();
    assert_eq!(
        c.rename_file("people/أحمد سمير.md", "people/أحمد سمير عبد الله.md"),
        1
    );
    assert_eq!(
        c.files(),
        [
            "notes/Pricing experiments.md",
            "people/أحمد سمير عبد الله.md"
        ]
    );
}

#[test]
fn block_id_append_on_fixture_keeps_prose() {
    let body = include_str!("fixtures/notes/Every link form.md");
    let a = analyze(body);
    let table = a
        .blocks
        .iter()
        .find(|b| body[b.span.clone()].starts_with("| Table"))
        .unwrap();
    let out = append_block_id(body, table.span.start, "tbl1").unwrap();
    assert_eq!(out.inserted, "\n\n^tbl1");
    assert_eq!(
        out.body.replacen(
            "| [[Note\\|table alias]] |\n\n^tbl1\n",
            "| [[Note\\|table alias]] |\n",
            1
        ),
        body
    );
    let a2 = analyze(&out.body);
    assert_eq!(
        a2.block_by_id("tbl1").map(|b| b.span.clone()),
        Some(table.span.clone())
    );
    // Every other block and link is unchanged.
    assert_eq!(a2.links.len(), a.links.len());
    assert_eq!(a2.blocks.len(), a.blocks.len());
}
