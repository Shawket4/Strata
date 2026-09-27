//! Body analysis, block-ID appends and AI-section replacement never panic, and appending an
//! ID never changes other bytes.
#![no_main]

use libfuzzer_sys::fuzz_target;
use vault_format::blocks::append_to;
use vault_format::sections::{AiProfile, AiSection, replace_ai_sections, sections};

fuzz_target!(|body: &str| {
    let a = vault_format::analyze(body);
    for block in &a.blocks {
        if let Ok(out) = append_to(body, block, "fz") {
            let mut restored = out.body.clone();
            restored.replace_range(out.at..out.at + out.inserted.len(), "");
            assert_eq!(restored, body);
        }
    }
    let _ = sections(body);
    let _ = replace_ai_sections(body, AiProfile::Entity, &[(AiSection::Summary, "S.")]);
});
