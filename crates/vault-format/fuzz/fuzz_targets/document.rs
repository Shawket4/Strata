//! Any text parses, renders back byte-for-byte, and canonical rendering is idempotent.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vault_format::Document;

fuzz_target!(|text: &str| {
    let doc = Document::parse(text);
    assert_eq!(doc.render(), text);
    let canonical = doc.render_canonical();
    let again = Document::parse(&canonical).render_canonical();
    assert_eq!(canonical, again);
    let _ = doc.analyze_body();
});
