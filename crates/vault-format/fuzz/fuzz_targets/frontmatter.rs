//! Arbitrary text as frontmatter: round-trips unchanged; when it is editable, a known key
//! can be written and reads back.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vault_format::{Document, KnownKey, PropertyValue};

fuzz_target!(|input: (&str, &str)| {
    let (yaml, value) = input;
    let text = format!("---\n{yaml}\n---\nbody\n");
    let mut doc = Document::parse(&text);
    assert_eq!(doc.render(), text);
    let editable = doc.frontmatter().is_some_and(|f| f.error().is_none());
    if editable {
        let fm = doc.frontmatter_mut();
        fm.set_text(KnownKey::Title, value).unwrap();
        fm.set_list(KnownKey::Aliases, vec![value.to_owned(), "x".to_owned()]).unwrap();
        let rendered = doc.render();
        let back = Document::parse(&rendered);
        let fm = back.frontmatter().unwrap();
        assert_eq!(fm.error(), None, "{rendered}");
        assert_eq!(fm.get("title"), Some(&PropertyValue::Text(value.to_owned())));
        assert_eq!(fm.aliases(), vec![value.to_owned(), "x".to_owned()]);
    }
});
