//! Properties of the slugging and path rules and of the rendered notes (PLAN §16: proptest for
//! parsers and path rules).
#![allow(clippy::expect_used, clippy::unwrap_used)]

use chrono::{DateTime, Utc};
use domain::NoteKind;
use item_render::capture::capture_content;
use item_render::entity::EntitySpec;
use item_render::note::{clean_list, with_id};
use item_render::paths::{
    capture_path, conflict_copy_path, entity_path, entity_stem, file_name, is_inbox_path, parent,
};
use proptest::prelude::*;
use ulid::Ulid;
use vault_format::Document;
use vault_format::filename::validate_vault_path;

const KINDS: [NoteKind; 5] = [
    NoteKind::Person,
    NoteKind::Company,
    NoteKind::Document,
    NoteKind::Place,
    NoteKind::Concept,
];

/// Names mixing Latin, Arabic, forbidden characters, dots, spaces and control characters.
fn name() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            "[a-zA-Z0-9]{1,6}",
            "[أحمدسميرعقدوطنية]{1,6}",
            prop::sample::select(vec![
                " ", "  ", ".", "..", ":", "/", "\\", "|", "?", "#", "^", "[", "]", "*", "\"", "<",
                ">", "\t", "\u{0}", "—", "CON", "nul", "é",
            ])
            .prop_map(str::to_owned),
        ],
        0..12,
    )
    .prop_map(|parts| parts.concat())
}

fn kind() -> impl Strategy<Value = NoteKind> {
    prop::sample::select(KINDS.to_vec())
}

fn created() -> impl Strategy<Value = DateTime<Utc>> {
    (
        0i64..4_102_444_800, // 1970 .. 2100
        0u32..1_000_000_000,
    )
        .prop_map(|(secs, nanos)| DateTime::from_timestamp(secs, nanos).unwrap())
}

fn stems_of<'a>(paths: &'a [String], folder: &'a str) -> impl Iterator<Item = String> + 'a {
    paths
        .iter()
        .filter(move |p| parent(p) == folder)
        .filter_map(|p| file_name(p).strip_suffix(".md"))
        .map(str::to_lowercase)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// An entity path is a valid vault path in the kind's folder, free among the taken
    /// notes of that folder (case-insensitively), and deterministic.
    #[test]
    fn entity_paths_are_valid_and_free(
        kind in kind(),
        name in name(),
        taken_names in prop::collection::vec(name(), 0..6),
        suffixes in prop::collection::vec(0u32..4, 0..6),
    ) {
        let folder = kind.default_folder();
        let stem = entity_stem(&name);
        let mut taken: Vec<String> = taken_names
            .iter()
            .map(|n| format!("{folder}/{}.md", entity_stem(n)))
            .collect();
        taken.extend(suffixes.iter().map(|n| match n {
            0 => format!("{folder}/{stem}.md"),
            1 => format!("{folder}/{} 2.md", stem.to_uppercase()),
            2 => format!("other/{stem} 3.md"),
            _ => format!("{folder}/sub/{stem} 3.md"),
        }));
        let path = entity_path(kind, &name, taken.iter().map(String::as_str));
        prop_assert_eq!(validate_vault_path(&path), Ok(()));
        prop_assert_eq!(parent(&path), folder);
        let got = file_name(&path).strip_suffix(".md").unwrap().to_owned();
        prop_assert!(got == stem || got.starts_with(&format!("{stem} ")), "{got:?} from {stem:?}");
        prop_assert!(!stems_of(&taken, folder).any(|s| s == got.to_lowercase()));
        prop_assert_eq!(entity_path(kind, &name, taken.iter().map(String::as_str)), path.clone());
        if taken.is_empty() {
            prop_assert_eq!(got, stem);
        }
    }

    /// Slugging is stable: a sanitised stem sanitises to itself.
    #[test]
    fn entity_stems_are_fixed_points(name in name()) {
        let stem = entity_stem(&name);
        prop_assert_eq!(entity_stem(&stem), stem.clone());
        prop_assert!(!stem.is_empty());
        prop_assert!(stem.len() <= 200);
    }

    /// A capture path is `inbox/YYYY-MM-DD-HHmmss[ N].md` in UTC, a valid inbox path, and
    /// free.
    #[test]
    fn capture_paths(created in created(), n in 0usize..4) {
        let stem = created.format("%Y-%m-%d-%H%M%S").to_string();
        let taken: Vec<String> = (0..n)
            .map(|i| if i == 0 { format!("inbox/{stem}.md") } else { format!("inbox/{stem} {}.md", i + 1) })
            .collect();
        let path = capture_path(&created, taken.iter().map(String::as_str));
        let expected = if n == 0 { format!("inbox/{stem}.md") } else { format!("inbox/{stem} {}.md", n + 1) };
        prop_assert_eq!(&path, &expected);
        prop_assert_eq!(validate_vault_path(&path), Ok(()));
        prop_assert!(is_inbox_path(&path));
    }

    /// A conflict copy sits next to its note, has a valid name and never equals the note or
    /// another copy.
    #[test]
    fn conflict_copies(kind in kind(), name in name(), secs in 0i64..4_102_444_800, n in 1u32..30) {
        let path = entity_path(kind, &name, []);
        let at = DateTime::from_timestamp(secs, 0).unwrap();
        let copy = conflict_copy_path(&path, &at, n);
        prop_assert_eq!(validate_vault_path(&copy), Ok(()));
        prop_assert_eq!(parent(&copy), parent(&path));
        prop_assert_ne!(&copy, &path);
        prop_assert_ne!(copy, conflict_copy_path(&path, &at, n + 1));
    }

    /// A capture reads back as its ID, its time in UTC whole seconds and the text (ending
    /// with a line break).
    #[test]
    fn captures_read_back(text in "[\\PC\n]{0,60}(\r\n[\\PC]{0,10})?", created in created(), raw in any::<u128>()) {
        let id = Ulid(raw);
        let content = capture_content(id, &created, &text).unwrap();
        let doc = Document::parse(&content);
        let fm = doc.frontmatter().unwrap();
        prop_assert_eq!(fm.id(), Ok(Some(id)));
        prop_assert_eq!(fm.created(), Ok(Some(DateTime::from_timestamp(created.timestamp(), 0).unwrap().fixed_offset())));
        prop_assert!(fm.text(vault_format::KnownKey::Created).unwrap().ends_with('Z'));
        let mut body = text.clone();
        if !body.ends_with('\n') {
            body.push('\n');
        }
        prop_assert_eq!(doc.body(), body);
    }

    /// `with_id` is idempotent and keeps the rest of the note.
    #[test]
    fn with_id_is_idempotent(body in "[\\PC\n]{0,60}", tags in prop::collection::vec("[a-z]{1,5}", 0..3), raw in any::<u128>()) {
        let id = Ulid(raw);
        let content = if tags.is_empty() { body.clone() } else { format!("---\ntags: [{}]\n---\n{body}", tags.join(", ")) };
        let once = with_id(&content, id).unwrap();
        prop_assert_eq!(with_id(&once, id).unwrap(), once.clone());
        let doc = Document::parse(&once);
        prop_assert_eq!(doc.frontmatter().unwrap().id(), Ok(Some(id)));
        prop_assert_eq!(doc.frontmatter().unwrap().tags(), tags);
    }

    /// An entity note reads back as its kind, ID, cleaned aliases, creation time and the title
    /// rule (a title exactly when the file stem — free or suffixed — differs from the name).
    #[test]
    fn entities_read_back(
        kind in kind(),
        name in name().prop_filter("a name", |n| !n.trim().is_empty() && !n.contains('\0')),
        aliases in prop::collection::vec("[a-zA-Zأحمد #]{0,8}", 0..4),
        taken in any::<bool>(),
        created in created(),
        raw in any::<u128>(),
    ) {
        let id = Ulid(raw);
        let spec = EntitySpec { aliases: aliases.clone(), ..EntitySpec::new(kind, name.clone()) };
        let taken_path = format!("{}/{}.md", kind.default_folder(), spec.stem());
        let path = spec.path(taken.then_some(taken_path.as_str()));
        let content = spec.render(id, &path, &created).unwrap();
        let doc = Document::parse(&content);
        let fm = doc.frontmatter().unwrap();
        prop_assert_eq!(fm.id(), Ok(Some(id)));
        prop_assert_eq!(fm.kind().and_then(|k| k.known()), Some(kind));
        prop_assert_eq!(fm.aliases(), clean_list(&aliases));
        let stem = file_name(&path).strip_suffix(".md").unwrap();
        prop_assert_eq!(taken, stem != spec.stem());
        let title = (stem != name.trim()).then(|| name.trim().to_owned());
        prop_assert_eq!(fm.title().map(str::to_owned), title);
        let written = DateTime::from_timestamp(created.timestamp(), 0).unwrap().fixed_offset();
        prop_assert_eq!(fm.created(), Ok(Some(written)));
        prop_assert_eq!(fm.updated(), Ok(Some(written)));
        prop_assert_eq!(doc.body(), "## Notes\n");
    }
}

#[test]
fn capture_paths_are_utc() {
    // Half past midnight in Cairo is still the previous day in UTC.
    let cairo = DateTime::parse_from_rfc3339("2026-09-27T00:30:00+03:00").unwrap();
    assert_eq!(
        capture_path(&cairo.to_utc(), []),
        "inbox/2026-09-26-213000.md"
    );
}
