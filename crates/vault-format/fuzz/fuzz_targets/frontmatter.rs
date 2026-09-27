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
        // Edits may be refused (when they would change how other entries read), but an
        // accepted edit must read back exactly.
        let fm = doc.frontmatter_mut();
        if fm.set_text(KnownKey::Title, value).is_err()
            || fm
                .set_list(KnownKey::Aliases, vec![value.to_owned(), "x".to_owned()])
                .is_err()
        {
            return;
        }
        let rendered = doc.render();
        let back = Document::parse(&rendered);
        let fm = back.frontmatter().unwrap();
        assert_eq!(fm.error(), None, "{rendered}");
        assert_eq!(
            fm.get("title"),
            Some(&PropertyValue::Text(value.to_owned()))
        );
        assert_eq!(fm.aliases(), vec![value.to_owned(), "x".to_owned()]);
    }
});
