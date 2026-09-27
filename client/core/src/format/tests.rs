use pretty_assertions::assert_eq;

use super::*;

const DOC: &str = "---\nid: 01J8ZK3M4X7Q9W2E5R6T8Y0V1H\nkind: document\naliases: [عقد وطنية]\ntags: [contracts]\ndoc-type: contract\ncopy: original\ncompanies: [\"[[Watanya]]\"]\nlocation: \"[[Safe — Nasr City office]]\"\nholder: \"\"\nlast-holder: \"[[Shady]]\"\nexpires: 2027-03-31\nstatus: stored\n---\n## Summary\nWatanya's contract. #legal\n## Custody\n- 2026-09-20 — returned-by [[Shady]] to [[Safe — Nasr City office]] — [[Capture 2026-09-20#^c1d2]]\n- 2026-03-02 — stored-at [[Safe — Nasr City office]] — [[Capture 2026-03-02]]\n## Notes\n- [ ] Renew 📅 2027-03-01 (@2027-03-01 09:00) ^t-r1\n";

#[test]
fn parses_a_document_note() {
    let p = parse_note("documents/Watanya contract.md", DOC);
    assert_eq!(p.title, "Watanya contract");
    assert_eq!(p.display_title, "Watanya contract");
    assert_eq!(p.kind, domain::NoteKind::Document);
    assert_eq!(p.aliases, vec!["عقد وطنية".to_owned()]);
    assert_eq!(p.tags, vec!["contracts".to_owned(), "legal".to_owned()]);
    assert_eq!(
        p.relations,
        vec![("companies".to_owned(), "Watanya".to_owned())]
    );
    assert_eq!(
        p.document,
        Some(DocumentFields {
            doc_type: Some("contract".into()),
            copy: Some("original".into()),
            copy_of: None,
            location: Some("Safe — Nasr City office".into()),
            holder: None,
            last_holder: Some("Shady".into()),
            status: Some("stored".into()),
            expires: Some("2027-03-31".into()),
        })
    );
    assert_eq!(p.custody.len(), 2);
    assert_eq!(p.custody[0].date.to_string(), "2026-09-20");
    assert_eq!(p.custody[0].person.as_deref(), Some("[[Shady]]"));
    assert_eq!(p.tasks.len(), 1);
    let t = &p.tasks[0];
    assert_eq!(t.block_id.as_deref(), Some("t-r1"));
    assert_eq!(t.line_no, 21);
    assert_eq!(t.description, "Renew");
    assert_eq!(t.due.map(|d| d.to_string()).as_deref(), Some("2027-03-01"));
    assert_eq!(t.reminders.len(), 1);
    assert_eq!(
        p.links
            .iter()
            .map(|l| (l.path.as_str(), l.anchor.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("Shady", None),
            ("Safe — Nasr City office", None),
            ("Capture 2026-09-20", Some("^c1d2")),
            ("Safe — Nasr City office", None),
            ("Capture 2026-03-02", None),
        ]
    );
}

#[test]
fn plain_note_without_frontmatter() {
    let p = parse_note("notes/Churn notes.md", "Churn is up [[Pricing]] ![[chart.png]]\n");
    assert_eq!(p.kind, domain::NoteKind::Note);
    assert_eq!(p.title, "Churn notes");
    assert!(p.properties.is_empty());
    assert_eq!(
        p.links,
        vec![
            LinkRef {
                path: "Pricing".into(),
                embed: false,
                anchor: None
            },
            LinkRef {
                path: "chart.png".into(),
                embed: true,
                anchor: None
            }
        ]
    );
}

#[test]
fn recurring_task_gets_an_rrule_and_bad_recurrence_is_flagged() {
    let p = parse_note(
        "tasks/Tasks.md",
        "- [ ] A 🔁 every month on the 1st 📅 2026-10-01 ^t-a\n- [ ] B 🔁 every blue moon 📅 2026-10-01 ^t-b\n",
    );
    assert_eq!(p.tasks[0].rrule.as_deref(), Some("FREQ=MONTHLY;BYMONTHDAY=1"));
    assert_eq!(p.tasks[0].recurrence_error, None);
    assert_eq!(p.tasks[1].rrule, None);
    assert_eq!(
        p.tasks[1].recurrence_raw.as_deref(),
        Some("every blue moon")
    );
    assert!(p.tasks[1].recurrence_error.is_some());
}

#[test]
fn place_parent_and_property_display() {
    let p = parse_note(
        "places/Safe — Nasr City office.md",
        "---\nkind: place\npart-of: [\"[[Nasr City office]]\"]\n---\n",
    );
    assert_eq!(p.kind, domain::NoteKind::Place);
    assert_eq!(p.place_parent.as_deref(), Some("Nasr City office"));
    assert_eq!(
        property_display(&PropertyValue::List(vec!["a".into(), "b".into()])),
        vec!["a".to_owned(), "b".to_owned()]
    );
    assert!(property_display(&PropertyValue::Null).is_empty());
}
