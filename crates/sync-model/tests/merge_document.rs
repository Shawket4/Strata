//! Document merge: frontmatter key by key, body diff3, conflict hunks and their resolution,
#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers outside #[test] fns
//! and the D19 update decision.

use pretty_assertions::assert_eq;
use sync_model::merge::{FmValue, HunkValues};
use sync_model::{
    AutoResolution, AutoResolved, Choice, ConflictHunk, ConflictKind, Location, MergeOutcome,
    ResolveError, UpdateDecision, Version, decide_update, merge,
};

fn note(fm: &str, body: &str) -> String {
    format!("---\n{fm}---\n{body}")
}

fn fm_loc(key: &str) -> Location {
    Location::Frontmatter { key: key.into() }
}

fn conflicted(o: MergeOutcome) -> sync_model::Conflicted {
    match o {
        MergeOutcome::Conflicted(c) => c,
        MergeOutcome::Clean(t) => panic!("expected a conflict, got clean {t:?}"),
    }
}

const ID: &str = "id: 01J8ZK3M4X7Q0000000000000A\n";

#[test]
fn lists_union_scalars_and_body_merge_cleanly() {
    let base = note(
        &format!(
            "{ID}title: Pricing\ntags: [pricing]\nrelated: [\"[[A]]\", \"[[B]]\"]\ncustom: keep me\nupdated: 2026-09-27T10:00:00+03:00\n"
        ),
        "line one\nline two\n",
    );
    let ours = note(
        &format!(
            "{ID}title: Pricing\ntags: [pricing, pos]\nrelated: [\"[[B]]\"]\ncustom: keep me\nupdated: 2026-09-27T11:00:00+03:00\n"
        ),
        "line one (ours)\nline two\n",
    );
    let theirs = note(
        &format!(
            "{ID}title: Pricing\ntags: [pricing, ai]\nrelated: [\"[[A]]\", \"[[B]]\", \"[[C]]\"]\ncustom: keep me\nupdated: 2026-09-27T12:00:00+03:00\n"
        ),
        "line one\nline two (theirs)\n",
    );
    assert_eq!(
        merge(&base, &ours, &theirs),
        MergeOutcome::Clean(note(
            &format!(
                "{ID}title: Pricing\ntags: [pricing, pos, ai]\nupdated: 2026-09-27T12:00:00+03:00\nrelated: [\"[[B]]\", \"[[C]]\"]\ncustom: keep me\n"
            ),
            "line one (ours)\nline two (theirs)\n",
        ))
    );
    // Symmetric up to list order.
    assert_eq!(
        merge(&base, &theirs, &ours),
        MergeOutcome::Clean(note(
            &format!(
                "{ID}title: Pricing\ntags: [pricing, ai, pos]\nupdated: 2026-09-27T12:00:00+03:00\nrelated: [\"[[B]]\", \"[[C]]\"]\ncustom: keep me\n"
            ),
            "line one (ours)\nline two (theirs)\n",
        ))
    );
}

#[test]
fn one_sided_key_changes_apply_and_unknown_keys_keep_order() {
    let base = note(&format!("{ID}zeta: 1\ntitle: Old\nalpha: a\n"), "body\n");
    let ours = note(&format!("{ID}zeta: 2\ntitle: Old\nalpha: a\n"), "body\n");
    let theirs = note(&format!("{ID}zeta: 1\ntitle: New\n"), "body\nmore\n");
    assert_eq!(
        merge(&base, &ours, &theirs),
        MergeOutcome::Clean(note(&format!("{ID}title: New\nzeta: 2\n"), "body\nmore\n"))
    );
}

#[test]
fn untouched_frontmatter_bytes_are_kept_when_only_one_side_edits_it() {
    let base = note(&format!("{ID}tags:   [ a,  b ]   # spaced\n"), "x\n");
    let ours = note(&format!("{ID}tags:   [ a,  b ]   # spaced\n"), "x\ny\n");
    let theirs = note(
        &format!("{ID}tags:   [ a,  b ]   # spaced\nlang: en\n"),
        "x\n",
    );
    assert_eq!(
        merge(&base, &ours, &theirs),
        MergeOutcome::Clean(note(
            &format!("{ID}tags:   [ a,  b ]   # spaced\nlang: en\n"),
            "x\ny\n"
        ))
    );
}

#[test]
fn scalar_conflict_keeps_ours_and_resolves_to_any_side() {
    let base = note(&format!("{ID}title: Pricing\n"), "same\n");
    let ours = note(&format!("{ID}title: Pricing tests\n"), "same\n");
    let theirs = note(&format!("{ID}title: Pricing experiments\n"), "same\nnew\n");
    let c = conflicted(merge(&base, &ours, &theirs));
    assert_eq!(c.merged_with_markers, None);
    assert_eq!(
        c.hunks,
        [ConflictHunk {
            id: 0,
            location: fm_loc("title"),
            kind: ConflictKind::BothModified,
            base: "title: Pricing\n".into(),
            ours: "title: Pricing tests\n".into(),
            theirs: "title: Pricing experiments\n".into(),
        }]
    );
    assert_eq!(
        c.template.values,
        [Some(HunkValues {
            base: FmValue::Text("Pricing".into()),
            ours: FmValue::Text("Pricing tests".into()),
            theirs: FmValue::Text("Pricing experiments".into()),
        })]
    );
    assert_eq!(
        c.auto_resolved,
        [AutoResolved {
            location: Location::Body {
                base_line: 1,
                ours_line: 1,
                theirs_line: 1
            },
            resolution: AutoResolution::TookTheirs
        }]
    );
    assert_eq!(
        c.resolve(&[(0, Choice::Ours)]),
        Ok(note(&format!("{ID}title: Pricing tests\n"), "same\nnew\n"))
    );
    assert_eq!(
        c.resolve(&[(0, Choice::Theirs)]),
        Ok(note(
            &format!("{ID}title: Pricing experiments\n"),
            "same\nnew\n"
        ))
    );
    assert_eq!(
        c.resolve(&[(0, Choice::Base)]),
        Ok(note(&format!("{ID}title: Pricing\n"), "same\nnew\n"))
    );
    assert_eq!(
        c.resolve(&[(0, Choice::Value(FmValue::Absent))]),
        Ok(note(ID, "same\nnew\n"))
    );
    assert_eq!(c.resolve(&[]), Err(ResolveError::Unresolved(0)));
    assert_eq!(
        c.resolve(&[(0, Choice::OursThenTheirs)]),
        Err(ResolveError::WrongChoice(0))
    );
    assert_eq!(
        c.resolve(&[(0, Choice::Ours), (1, Choice::Ours)]),
        Err(ResolveError::UnknownHunk(1))
    );
}

#[test]
fn delete_versus_modify_of_a_key_conflicts() {
    let base = note(&format!("{ID}role: Ops\n"), "b\n");
    let ours = note(ID, "b\n");
    let theirs = note(&format!("{ID}role: Operations manager\n"), "b\n");
    let c = conflicted(merge(&base, &ours, &theirs));
    assert_eq!(c.hunks.len(), 1);
    assert_eq!(
        (c.hunks[0].ours.as_str(), c.hunks[0].theirs.as_str()),
        ("", "role: Operations manager\n")
    );
    assert_eq!(
        c.resolve(&[(0, Choice::Theirs)]),
        Ok(note(&format!("{ID}role: Operations manager\n"), "b\n"))
    );
}

#[test]
fn key_deleted_on_one_side_stays_deleted() {
    let base = note(&format!("{ID}lang: en\nsource: \"[[x.m4a]]\"\n"), "b\n");
    let ours = note(&format!("{ID}lang: en\n"), "b\n");
    let theirs = note(&format!("{ID}lang: ar\nsource: \"[[x.m4a]]\"\n"), "b\n");
    assert_eq!(
        merge(&base, &ours, &theirs),
        MergeOutcome::Clean(note(&format!("{ID}lang: ar\n"), "b\n"))
    );
}

#[test]
fn list_deletions_on_both_sides_and_arabic_aliases() {
    let base = note(
        &format!("{ID}kind: person\naliases: [أحمد سمير, Ahmed S.]\n"),
        "## Notes\n",
    );
    let ours = note(
        &format!("{ID}kind: person\naliases: [أحمد سمير, A. Samir]\n"),
        "## Notes\n",
    );
    let theirs = note(
        &format!("{ID}kind: person\naliases: [Ahmed S., احمد سمير]\n"),
        "## Notes\n",
    );
    assert_eq!(
        merge(&base, &ours, &theirs),
        MergeOutcome::Clean(note(
            &format!("{ID}kind: person\naliases: [A. Samir, احمد سمير]\n"),
            "## Notes\n"
        ))
    );
    // Removing every entry on both sides removes the key.
    let ours = note(
        &format!("{ID}kind: person\naliases: [Ahmed S.]\n"),
        "## Notes\n",
    );
    let theirs = note(
        &format!("{ID}kind: person\naliases: [أحمد سمير]\n"),
        "## Notes\n",
    );
    assert_eq!(
        merge(&base, &ours, &theirs),
        MergeOutcome::Clean(note(
            &format!("{ID}kind: person\naliases: []\n"),
            "## Notes\n"
        ))
    );
}

#[test]
fn id_changed_differently_conflicts() {
    let base = note(ID, "b\n");
    let ours = note("id: 01J8ZK3M4X7Q0000000000000B\n", "b\n");
    let theirs = note("id: 01J8ZK3M4X7Q0000000000000C\n", "b\n");
    let c = conflicted(merge(&base, &ours, &theirs));
    assert_eq!(c.hunks[0].location, fm_loc("id"));
}

#[test]
fn updated_takes_the_later_time_across_offsets() {
    let base = note("updated: 2026-09-27T10:00:00+03:00\n", "b\n");
    let ours = note("updated: 2026-09-27T09:30:00Z\n", "b\nours\n");
    let theirs = note("updated: 2026-09-27T12:00:00+03:00\n", "b\n");
    // 09:30Z = 12:30+03:00 is later than 12:00+03:00.
    assert_eq!(
        merge(&base, &ours, &theirs),
        MergeOutcome::Clean(note("updated: 2026-09-27T09:30:00Z\n", "b\nours\n"))
    );
}

#[test]
fn body_conflict_with_clean_frontmatter_has_markers_and_resolves() {
    let base = note(
        &format!("{ID}tags: [a]\n"),
        "intro\n- [ ] pay rent ^t-1\nend\n",
    );
    let ours = note(
        &format!("{ID}tags: [a, b]\n"),
        "intro\n- [x] pay rent ✅ 2026-09-27 ^t-1\nend\n",
    );
    let theirs = note(
        &format!("{ID}tags: [a]\n"),
        "intro\n- [ ] pay the rent ^t-1\nend\n",
    );
    let c = conflicted(merge(&base, &ours, &theirs));
    assert_eq!(
        c.merged_with_markers.as_deref(),
        Some(
            note(
                &format!("{ID}tags: [a, b]\n"),
                "intro\n<<<<<<< ours\n- [x] pay rent ✅ 2026-09-27 ^t-1\n||||||| base\n- [ ] pay rent ^t-1\n=======\n- [ ] pay the rent ^t-1\n>>>>>>> theirs\nend\n"
            )
            .as_str()
        )
    );
    assert_eq!(
        c.hunks,
        [ConflictHunk {
            id: 0,
            location: Location::Body {
                base_line: 1,
                ours_line: 1,
                theirs_line: 1
            },
            kind: ConflictKind::TaskLine,
            base: "- [ ] pay rent ^t-1\n".into(),
            ours: "- [x] pay rent ✅ 2026-09-27 ^t-1\n".into(),
            theirs: "- [ ] pay the rent ^t-1\n".into(),
        }]
    );
    assert_eq!(
        c.resolve(&[(
            0,
            Choice::Text("- [x] pay the rent ✅ 2026-09-27 ^t-1\n".into())
        )]),
        Ok(note(
            &format!("{ID}tags: [a, b]\n"),
            "intro\n- [x] pay the rent ✅ 2026-09-27 ^t-1\nend\n"
        ))
    );
    assert_eq!(
        c.resolve(&[(0, Choice::TheirsThenOurs)]),
        Ok(note(
            &format!("{ID}tags: [a, b]\n"),
            "intro\n- [ ] pay the rent ^t-1\n- [x] pay rent ✅ 2026-09-27 ^t-1\nend\n"
        ))
    );
    assert_eq!(
        c.resolve(&[(0, Choice::Value(FmValue::Null))]),
        Err(ResolveError::WrongChoice(0))
    );
}

#[test]
fn frontmatter_and_body_hunks_are_numbered_in_order() {
    let base = note(&format!("{ID}title: T\n"), "a\n");
    let ours = note(&format!("{ID}title: T1\n"), "b\n");
    let theirs = note(&format!("{ID}title: T2\n"), "c\n");
    let c = conflicted(merge(&base, &ours, &theirs));
    let got: Vec<(u32, Location)> = c.hunks.iter().map(|h| (h.id, h.location.clone())).collect();
    assert_eq!(
        got,
        [
            (0, fm_loc("title")),
            (
                1,
                Location::Body {
                    base_line: 0,
                    ours_line: 0,
                    theirs_line: 0
                }
            )
        ]
    );
    assert_eq!(
        c.resolve(&[(1, Choice::Base), (0, Choice::Theirs)]),
        Ok(note(&format!("{ID}title: T2\n"), "a\n"))
    );
}

#[test]
fn invalid_yaml_falls_back_to_a_whole_file_merge() {
    let base = "---\ntitle: [unclosed\n---\na\nb\n";
    let ours = "---\ntitle: [unclosed\n---\nA\nb\n";
    let theirs = "---\ntitle: [unclosed\n---\na\nB\n";
    assert_eq!(
        merge(base, ours, theirs),
        MergeOutcome::Clean("---\ntitle: [unclosed\n---\nA\nB\n".into())
    );
}

#[test]
fn notes_without_frontmatter_merge_as_bodies() {
    assert_eq!(
        merge("a\nb\n", "a\nb\nc\n", "z\na\nb\n"),
        MergeOutcome::Clean("z\na\nb\nc\n".into())
    );
    // One side adds frontmatter.
    assert_eq!(
        merge("a\n", &note("tags: [x]\n", "a\n"), "a\nb\n"),
        MergeOutcome::Clean(note("tags: [x]\n", "a\nb\n"))
    );
}

#[test]
fn crlf_documents_merge() {
    let base = "---\r\ntags: [a]\r\n---\r\none\r\ntwo\r\n";
    let ours = "---\r\ntags: [a, b]\r\n---\r\nONE\r\ntwo\r\n";
    let theirs = "---\r\ntags: [a]\r\n---\r\none\r\nTWO\r\n";
    assert_eq!(
        merge(base, ours, theirs),
        MergeOutcome::Clean("---\r\ntags: [a, b]\r\n---\r\nONE\r\nTWO\r\n".into())
    );
}

#[test]
fn decide_update_policy() {
    let base = note(ID, "a\nb\n");
    let current = note(ID, "a\nb\nserver\n");
    let edit = note(ID, "client\na\nb\n");
    let v_base = Version::of_text(&base);
    assert_eq!(
        decide_update(&Version::of_text(&current), Some(&current), &current, &edit),
        UpdateDecision::FastForward
    );
    assert_eq!(
        decide_update(&v_base, Some(&base), &current, &current),
        UpdateDecision::AlreadyApplied
    );
    assert_eq!(
        decide_update(&v_base, Some(&base), &current, &edit),
        UpdateDecision::Merged(note(ID, "client\na\nb\nserver\n"))
    );
    let conflicting = note(ID, "a\nb\nclient\n");
    let UpdateDecision::Conflict(c) = decide_update(&v_base, Some(&base), &current, &conflicting)
    else {
        panic!("expected a conflict");
    };
    assert_eq!(
        (c.hunks[0].ours.as_str(), c.hunks[0].theirs.as_str()),
        ("server\n", "client\n")
    );
    // Edit already contained in the current version.
    assert_eq!(
        decide_update(&v_base, Some(&base), &note(ID, "client\na\nb\n"), &edit),
        UpdateDecision::AlreadyApplied
    );
}

#[test]
fn outcomes_round_trip_as_named_msgpack() {
    let c = merge(
        &note(&format!("{ID}title: T\n"), "a\n"),
        &note(&format!("{ID}title: T1\n"), "b\n"),
        &note(&format!("{ID}title: T2\n"), "c\n"),
    );
    let bytes = rmp_serde::to_vec_named(&c).unwrap();
    let back: MergeOutcome = rmp_serde::from_slice(&bytes).unwrap();
    assert_eq!(back, c);
}
