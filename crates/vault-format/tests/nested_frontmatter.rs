//! Nested frontmatter values: `set_yaml` (canonical block YAML) and `set_raw_entry`
//! (verbatim entries), byte preservation of everything else, and read-back verification.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use pretty_assertions::assert_eq;
use proptest::prelude::*;
use vault_format::frontmatter::Yaml;
use vault_format::{Document, FrontmatterError, PropertyValue};

fn s(v: &str) -> Yaml {
    Yaml::String(v.to_owned())
}

fn map(items: Vec<(Yaml, Yaml)>) -> Yaml {
    Yaml::Hash(items.into_iter().collect())
}

fn invalid(key: &str, reason: &str) -> FrontmatterError {
    FrontmatterError::InvalidValue {
        key: key.to_owned(),
        reason: reason.to_owned(),
    }
}

const DOC: &str = "---\nid: 01J8ZK3M4X7Q0000000000000A\ntitle: Plugin settings\n# kept with its entry\nplugin:\n  a: 1\nlast: [z]\n---\nنص عربي\nbody\n";

fn sample_value() -> Yaml {
    map(vec![
        (s("a"), Yaml::Integer(2)),
        (s("list"), Yaml::Array(vec![s("x"), s("y: z"), s("أحمد")])),
        (
            s("deep"),
            map(vec![(
                s("k"),
                Yaml::Array(vec![
                    map(vec![(s("nr"), Yaml::Integer(1)), (s("m"), s("مريم"))]),
                    Yaml::Array(vec![]),
                    Yaml::Array(vec![s("in"), Yaml::Boolean(true)]),
                ]),
            )]),
        ),
        (s("empty"), map(vec![])),
        (s("none"), Yaml::Null),
        (s("quoted key: yes"), s("")),
    ])
}

const SAMPLE_RAW: &str = "plugin:\n  a: 2\n  list:\n    - x\n    - \"y: z\"\n    - أحمد\n  deep:\n    k:\n      - nr: 1\n        m: مريم\n      - []\n      - - in\n        - true\n  empty: {}\n  none: null\n  \"quoted key: yes\": \"\"\n";

#[test]
fn set_yaml_writes_block_yaml_and_keeps_every_other_byte() {
    let mut doc = Document::parse(DOC);
    let fm = doc.frontmatter_mut();
    fm.set_yaml("plugin", &sample_value()).unwrap();
    assert_eq!(fm.raw_entry("plugin"), Some(SAMPLE_RAW));
    assert_eq!(fm.get("plugin"), Some(&PropertyValue::Other));
    let expected = DOC.replace("plugin:\n  a: 1\n", SAMPLE_RAW);
    assert_eq!(doc.render(), expected);
    assert_eq!(doc.render_canonical(), expected);

    let reparsed = Document::parse(&expected);
    let fm = reparsed.frontmatter().unwrap();
    assert_eq!(fm.error(), None);
    assert_eq!(fm.yaml("plugin"), Some(&sample_value()));
    assert_eq!(fm.get("last"), Some(&PropertyValue::List(vec!["z".into()])));
}

#[test]
fn equal_nested_value_changes_nothing() {
    let text = "---\nplugin: {a: 1}   # flow style, kept\n---\n";
    let mut doc = Document::parse(text);
    let fm = doc.frontmatter_mut();
    fm.set_yaml("plugin", &map(vec![(s("a"), Yaml::Integer(1))]))
        .unwrap();
    assert!(!fm.is_modified());
    assert_eq!(doc.render(), text);
}

#[test]
fn new_nested_key_is_appended_after_known_keys() {
    let mut doc = Document::parse("---\r\ncustom: 1\r\nid: 01J\r\n---\r\n");
    let fm = doc.frontmatter_mut();
    fm.set_yaml(
        "people-meta",
        &Yaml::Array(vec![map(vec![
            (s("name"), s("Shady")),
            (s("roles"), Yaml::Array(vec![s("owner"), s("2024")])),
        ])]),
    )
    .unwrap();
    assert_eq!(
        doc.render(),
        "---\r\nid: 01J\r\ncustom: 1\r\npeople-meta:\r\n  - name: Shady\r\n    roles:\r\n      - owner\r\n      - \"2024\"\r\n---\r\n"
    );
}

#[test]
fn flat_values_through_set_yaml_read_as_properties() {
    let mut doc = Document::parse("---\nid: 01J\n---\n");
    let fm = doc.frontmatter_mut();
    fm.set_yaml("tags", &Yaml::Array(vec![s("a"), s("b c")]))
        .unwrap();
    fm.set_yaml("title", &s("007")).unwrap();
    fm.set_yaml("empty", &Yaml::Null).unwrap();
    assert_eq!(
        fm.get("tags"),
        Some(&PropertyValue::List(vec!["a".into(), "b c".into()]))
    );
    assert_eq!(fm.get("title"), Some(&PropertyValue::Text("007".into())));
    assert_eq!(fm.get("empty"), Some(&PropertyValue::Null));
    assert_eq!(
        doc.render(),
        "---\nid: 01J\ntitle: \"007\"\ntags:\n  - a\n  - b c\nempty:\n---\n"
    );
}

#[test]
fn nested_value_can_be_replaced_by_a_flat_one_and_back() {
    let mut doc = Document::parse(DOC);
    let fm = doc.frontmatter_mut();
    fm.set("plugin", PropertyValue::Text("off".into())).unwrap();
    assert_eq!(fm.yaml("plugin"), Some(&s("off")));
    // `off` would read as a boolean in YAML 1.1 tools, so it is quoted.
    assert_eq!(
        doc.render(),
        DOC.replace("plugin:\n  a: 1\n", "plugin: \"off\"\n")
    );
    let fm = doc.frontmatter_mut();
    fm.set_yaml("plugin", &map(vec![(s("a"), Yaml::Integer(1))]))
        .unwrap();
    assert_eq!(doc.render(), DOC);
}

#[test]
fn unwritable_values_are_refused_without_change() {
    let mut doc = Document::parse(DOC);
    let fm = doc.frontmatter_mut();
    assert_eq!(
        fm.set_yaml("plugin", &Yaml::Array(vec![Yaml::Alias(0)])),
        Err(invalid("plugin", "aliases cannot be written"))
    );
    assert_eq!(
        fm.set_yaml("plugin", &Yaml::BadValue),
        Err(invalid("plugin", "bad value"))
    );
    assert_eq!(
        fm.set_yaml(
            "plugin",
            &map(vec![(Yaml::Array(vec![s("k")]), Yaml::Integer(1))])
        ),
        Err(invalid("plugin", "mapping keys must be scalars"))
    );
    // `set` still writes flat values only.
    assert_eq!(
        fm.set("plugin", PropertyValue::Other),
        Err(FrontmatterError::NestedValue("plugin".into()))
    );
    assert!(!fm.is_modified());
    assert_eq!(doc.render(), DOC);
}

#[test]
fn edits_that_change_other_values_are_refused() {
    // `b` aliases the anchor on `a`: replacing `a` would change how `b` reads.
    let text = "---\na: &x\n  k: 1\nb: *x\n---\n";
    let mut doc = Document::parse(text);
    let fm = doc.frontmatter_mut();
    assert_eq!(fm.yaml("b"), Some(&map(vec![(s("k"), Yaml::Integer(1))])));
    let err = FrontmatterError::Unsupported(
        "the edit would change how the other properties are read".into(),
    );
    assert_eq!(
        fm.set_yaml("a", &map(vec![(s("k"), Yaml::Integer(2))])),
        Err(err.clone())
    );
    assert_eq!(fm.set_raw_entry("a", "a:\n  k: 2\n"), Err(err));
    assert_eq!(doc.render(), text);
}

#[test]
fn invalid_frontmatter_refuses_nested_edits() {
    let text = "---\na: [\n---\n";
    let mut doc = Document::parse(text);
    let fm = doc.frontmatter_mut();
    assert!(matches!(
        fm.set_yaml("b", &Yaml::Null),
        Err(FrontmatterError::InvalidYaml(_))
    ));
    assert!(matches!(
        fm.set_raw_entry("b", "b: 1\n"),
        Err(FrontmatterError::InvalidYaml(_))
    ));
    assert_eq!(fm.yaml("a"), None);
    assert_eq!(doc.render(), text);
}

#[test]
fn raw_entries_are_written_verbatim_with_this_files_line_endings() {
    let theirs = Document::parse(
        "---\r\nplugin:\r\n  # their comment\r\n  a: 1   # trailing\r\n\r\n  b: [x, y]\r\n---\r\n",
    );
    let raw = theirs.frontmatter().unwrap().raw_entry("plugin").unwrap();
    assert_eq!(
        raw,
        "plugin:\r\n  # their comment\r\n  a: 1   # trailing\r\n\r\n  b: [x, y]\r\n"
    );
    let mut doc = Document::parse(DOC);
    let fm = doc.frontmatter_mut();
    fm.set_raw_entry("plugin", raw).unwrap();
    let expected_raw = "plugin:\n  # their comment\n  a: 1   # trailing\n\n  b: [x, y]\n";
    assert_eq!(fm.raw_entry("plugin"), Some(expected_raw));
    assert_eq!(
        fm.yaml("plugin"),
        Some(&map(vec![
            (s("a"), Yaml::Integer(1)),
            (s("b"), Yaml::Array(vec![s("x"), s("y")])),
        ]))
    );
    assert_eq!(doc.render(), DOC.replace("plugin:\n  a: 1\n", expected_raw));

    // Same entry again: nothing changes. Missing final terminator: added.
    let mut doc = Document::parse(DOC);
    let fm = doc.frontmatter_mut();
    fm.set_raw_entry("plugin", "plugin:\n  a: 1").unwrap();
    assert!(!fm.is_modified());
    fm.set_raw_entry("new", "new:\n  - [a, b]").unwrap();
    assert_eq!(
        doc.render(),
        DOC.replace("last: [z]\n", "last: [z]\nnew:\n  - [a, b]\n")
    );
}

#[test]
fn raw_entries_must_be_one_entry_for_the_key() {
    let mut doc = Document::parse(DOC);
    let fm = doc.frontmatter_mut();
    let single = invalid("plugin", "not a single entry for this key");
    for bad in [
        "other: 1\n",
        "plugin: 1\nother: 2\n",
        "# comment\nplugin: 1\n",
        "plugin: 1\n\n",
        "plugin: 1\n# trailing\n",
        "  indented: 1\n",
        "",
    ] {
        assert_eq!(
            fm.set_raw_entry("plugin", bad),
            Err(single.clone()),
            "{bad:?}"
        );
    }
    assert_eq!(
        fm.set_raw_entry("plugin", "plugin: [\n"),
        Err(invalid("plugin", "not valid YAML on its own"))
    );
    assert_eq!(
        fm.set_raw_entry("plugin", "plugin: a\rb\n"),
        Err(invalid("plugin", "bare carriage return"))
    );
    assert_eq!(doc.render(), DOC);
}

// ---------------------------------------------------------------------------------------
// Properties

fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-zA-Z0-9 ._-]{0,10}",
        "[ء-ي ]{1,10}",
        "[a-zأ-ي0-9 :#,\\[\\]{}\"'|>&*!%@`?\\\\-]{0,10}",
        Just("true".to_owned()),
        Just("null".to_owned()),
        Just("12.50".to_owned()),
        Just("line\nbreak\ttab".to_owned()),
        "[a-zA-Z أ-ي]{1,8}".prop_map(|s| format!("[[{s}]]")),
    ]
}

fn scalar_node() -> impl Strategy<Value = Yaml> {
    prop_oneof![
        text().prop_map(Yaml::String),
        any::<i32>().prop_map(|i| Yaml::Integer(i64::from(i))),
        any::<bool>().prop_map(Yaml::Boolean),
        Just(Yaml::Null),
        Just(Yaml::Real("1.5".into())),
    ]
}

fn node() -> impl Strategy<Value = Yaml> {
    scalar_node().prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Yaml::Array),
            prop::collection::vec((text(), inner), 0..4).prop_map(|kv| Yaml::Hash(
                kv.into_iter().map(|(k, v)| (Yaml::String(k), v)).collect()
            )),
        ]
    })
}

const PREFIX: &str =
    "---\nid: 01J8ZK3M4X7Q0000000000000A\ntitle: T\n# comment\nextra: [a, \"b, c\"]\n";
const SUFFIX: &str = "last: |\n  block\n  text\n---\nbody ^b1\n";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any nested value round-trips through `set_yaml`; every other byte is kept, and the
    /// written entry copied with `set_raw_entry` into a CRLF note reads back the same.
    #[test]
    fn nested_set_round_trips(value in node(), old in node()) {
        let mut base = Document::parse(PREFIX);
        base.frontmatter_mut().set_yaml("nested", &old).unwrap();
        let old_raw = base.frontmatter().unwrap().raw_entry("nested").unwrap();
        let start = format!("{PREFIX}{old_raw}{SUFFIX}");
        let mut doc = Document::parse(&start);
        prop_assert_eq!(doc.frontmatter().unwrap().yaml("nested"), Some(&old));
        let fm = doc.frontmatter_mut();
        fm.set_yaml("nested", &value).unwrap();
        let raw = fm.raw_entry("nested").unwrap().to_owned();
        let rendered = doc.render();
        prop_assert_eq!(&rendered, &format!("{PREFIX}{raw}{SUFFIX}"));
        let back = Document::parse(&rendered);
        let fm = back.frontmatter().unwrap();
        prop_assert_eq!(fm.error(), None);
        prop_assert_eq!(fm.yaml("nested"), Some(&value));
        prop_assert_eq!(fm.get("last"), Some(&PropertyValue::Text("block\ntext\n".into())));

        let mut crlf = Document::parse("---\r\nid: 01J\r\n---\r\nx\r\n");
        crlf.frontmatter_mut().set_raw_entry("nested", &raw).unwrap();
        let crlf_text = crlf.render();
        prop_assert_eq!(&crlf_text, &format!("---\r\nid: 01J\r\n{}---\r\nx\r\n", raw.replace('\n', "\r\n")));
        let crlf_back = Document::parse(&crlf_text);
        prop_assert_eq!(crlf_back.frontmatter().unwrap().yaml("nested"), Some(&value));
    }
}
