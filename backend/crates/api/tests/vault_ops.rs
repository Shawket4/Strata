//! Export, import and integrity over HTTP (PLAN §6.10, §7.3, §16.3 Vault store): an
//! Obsidian vault imports in one revertible commit and export → import → export is
//! byte-identical; hostile archives are rejected whole; reconciliation warnings are listed.
#![allow(clippy::expect_used, clippy::too_many_lines)]

mod vault_harness;

use std::io::{Cursor, Write};

use pretty_assertions::assert_eq;
use strata_client::{operations as ops, types};
use vault_harness::{H, plain};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../crates/vault-format/tests/fixtures");

fn fixture(rel: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{rel}")).expect("fixture")
}

fn options() -> zip::write::SimpleFileOptions {
    zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        z.start_file(*name, options()).expect("entry");
        z.write_all(bytes).expect("write");
    }
    z.finish().expect("zip").into_inner()
}

/// A small but messy Obsidian vault: hand-written frontmatter, both scripts, CRLF, tasks
/// without block IDs, a canvas, a binary attachment and Obsidian's own config.
fn obsidian_vault() -> Vec<u8> {
    let png: Vec<u8> = (0u8..=255).chain([0x89, b'P', b'N', b'G', 0, 0xff]).collect();
    zip_of(&[
        (".obsidian/app.json", b"{\"legacyEditor\": false}"),
        (".obsidian/workspace.json", b"{}"),
        ("notes/Obsidian messy.md", &fixture("notes/Obsidian messy.md")),
        ("notes/Every link form.md", &fixture("notes/Every link form.md")),
        ("notes/ملاحظة مختلطة CRLF.md", &fixture("notes/ملاحظة مختلطة CRLF.md")),
        ("people/أحمد سمير.md", &fixture("people/أحمد سمير.md")),
        ("documents/Watanya contract.md", &fixture("documents/Watanya contract.md")),
        ("maps/Pricing.canvas", &fixture("maps/Pricing.canvas")),
        (
            "projects/Alpha/Kickoff.md",
            b"---\r\nstatus: active\r\n---\r\n# Kickoff\r\n\r\n- [ ] Book the room\r\n- [x] Send invites\r\n",
        ),
        ("attachments/diagram.png", &png),
    ])
}

#[tokio::test]
async fn obsidian_vault_imports_revertibly_and_round_trips_byte_identical() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let bob = h.user("bob").await;
    let before = ops::export_vault(&alice.client).await.expect("empty export");

    let report = ops::import_vault(&alice.client, obsidian_vault().into())
        .await
        .expect("import");
    let commit = report.commit.clone().expect("commit");
    assert_eq!(
        report.skipped,
        vec![".obsidian/app.json".to_owned(), ".obsidian/workspace.json".to_owned()]
    );
    assert_eq!(
        report.imported,
        vec![
            "attachments/diagram.png",
            "documents/Watanya contract.md",
            "maps/Pricing.canvas",
            "notes/Every link form.md",
            "notes/Obsidian messy.md",
            "notes/ملاحظة مختلطة CRLF.md",
            "people/أحمد سمير.md",
            "projects/Alpha/Kickoff.md",
        ]
    );
    assert_eq!(h.log(alice.id)[0], "user: import 8 files");
    assert_eq!(h.log(alice.id).len(), 2, "one commit");
    // Files that already had an ID keep every byte; binaries are untouched.
    assert_eq!(
        std::fs::read(h.dir(alice.id).join("notes/Obsidian messy.md")).expect("read"),
        fixture("notes/Obsidian messy.md")
    );
    assert_eq!(
        std::fs::read(h.dir(alice.id).join("maps/Pricing.canvas")).expect("read"),
        fixture("maps/Pricing.canvas")
    );
    let kickoff = h.read(alice.id, "projects/Alpha/Kickoff.md");
    assert!(report.ids_assigned.contains(&"projects/Alpha/Kickoff.md".to_owned()));
    assert!(kickoff.contains("status: active"), "{kickoff}");
    assert!(kickoff.contains("- [ ] Book the room ^t-"), "{kickoff}");
    let found = ops::search(&alice.client, "Kickoff", None, None).await.expect("search");
    assert_eq!(
        found.hits.iter().map(|i| i.path.clone()).collect::<Vec<_>>(),
        vec!["projects/Alpha/Kickoff.md".to_owned()]
    );

    // Export is deterministic; importing it elsewhere and exporting again is identical.
    let first = ops::export_vault(&alice.client).await.expect("export");
    let again = ops::export_vault(&alice.client).await.expect("export");
    assert_eq!(first, again);
    let into_bob = ops::import_vault(&bob.client, first.clone()).await.expect("import");
    assert_eq!(into_bob.ids_assigned, Vec::<String>::new());
    assert_eq!(into_bob.skipped, vec![".obsidian/app.json".to_owned()]);
    let second = ops::export_vault(&bob.client).await.expect("export");
    assert!(first == second, "export → import → export is byte-identical");
    // Re-importing the same export into the same vault changes nothing.
    let noop = ops::import_vault(&alice.client, first.clone()).await.expect("import");
    assert_eq!(noop.commit, None);
    assert_eq!(h.log(alice.id).len(), 2);

    // Reverting the import commit restores the vault as it was.
    let reverted = ops::revert_commit(&alice.client, &commit).await.expect("revert");
    assert_eq!(h.log(alice.id)[0], reverted_message(&reverted, &h, alice.id));
    let after = ops::export_vault(&alice.client).await.expect("export");
    assert!(after == before, "the revert undoes the whole import");
    assert_eq!(
        ops::search(&alice.client, "Kickoff", None, None)
            .await
            .expect("search")
            .hits,
        vec![]
    );
    h.finish().await;
}

fn reverted_message(_r: &types::CommitReverted, h: &H, user: strata_common::UserId) -> String {
    let msg = h.log(user)[0].clone();
    assert!(msg.starts_with("user: revert "), "{msg}");
    msg
}

fn invalid(detail: &str) -> types::Problem {
    plain("invalid_archive", "Invalid archive", 422, Some(detail))
}

#[tokio::test]
async fn hostile_archives_are_rejected_whole() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let ok: &[u8] = b"fine\n";
    let cases: Vec<(Vec<u8>, types::Problem)> = vec![
        (
            zip_of(&[("notes/ok.md", ok), ("../evil.md", ok)]),
            invalid("an entry path contains .."),
        ),
        (
            zip_of(&[("notes/ok.md", ok), ("notes/../../evil.md", ok)]),
            invalid("an entry path contains .."),
        ),
        (
            zip_of(&[("/etc/cron.d/evil.md", ok)]),
            invalid("an entry has an absolute path"),
        ),
        (
            zip_of(&[("notes\\..\\evil.md", ok)]),
            invalid("an entry path contains a backslash"),
        ),
        (zip_of(&[("C:/evil.md", ok)]), invalid("an entry path has a drive prefix")),
        (
            {
                let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
                z.start_file("notes/ok.md", options()).expect("entry");
                z.write_all(ok).expect("write");
                z.add_symlink("notes/link.md", "/etc/passwd", options())
                    .expect("symlink");
                z.finish().expect("zip").into_inner()
            },
            invalid("the archive contains a symlink"),
        ),
        (
            b"PK\x03\x04 definitely not a zip".to_vec(),
            invalid("the body is not a zip archive"),
        ),
        (
            {
                // 65 MiB of zeros compresses to a few KiB: stopped by the entry limit.
                let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
                z.start_file("notes/bomb.md", options().large_file(true))
                    .expect("entry");
                let chunk = vec![0u8; 1024 * 1024];
                for _ in 0..65 {
                    z.write_all(&chunk).expect("write");
                }
                z.finish().expect("zip").into_inner()
            },
            plain(
                "payload_too_large",
                "Request body too large",
                413,
                Some("an entry is larger than 67108864 bytes"),
            ),
        ),
    ];
    for (body, expected) in cases {
        let err = ops::import_vault(c, body.into()).await.expect_err("rejected");
        assert_eq!(vault_harness::problem(&err), expected);
        assert_eq!(h.log(alice.id).len(), 1, "nothing written");
    }
    // Wrong media type → 415 (raw: the client always sends application/zip).
    let (status, _, body) = h
        .raw(
            &alice.token,
            reqwest::Method::POST,
            "/api/v1/import",
            Some("import_vault"),
            &[],
            Some(("application/vnd.msgpack", zip_of(&[("notes/ok.md", ok)]))),
        )
        .await;
    assert_eq!(status, 415, "{}", String::from_utf8_lossy(&body));
    assert_eq!(h.log(alice.id).len(), 1);
    assert_eq!(
        std::fs::exists(h.dir(alice.id).join("../evil.md")).expect("exists"),
        false
    );
    h.finish().await;
}

#[tokio::test]
async fn out_of_band_edits_are_reconciled_and_reported() {
    let h = H::new().await;
    let alice = h.user("alice").await;
    let c = &alice.client;
    let n = ops::create_note(
        c,
        &types::CreateNoteRequest {
            content: "Original.\n".into(),
            force: None,
            id: None,
            path: "notes/Edited.md".into(),
        },
    )
    .await
    .expect("note");
    assert_eq!(ops::get_integrity(c).await.expect("integrity").warnings, vec![]);
    // Obsidian (or a crash) touched the vault while the server was away.
    let dir = h.dir(alice.id);
    std::fs::write(dir.join("notes/Outside.md"), "Written by Obsidian.\n").expect("write");
    let edited = h.read(alice.id, "notes/Edited.md").replace("Original.", "Edited outside.");
    std::fs::write(dir.join("notes/Edited.md"), edited).expect("write");
    std::fs::write(dir.join("notes/.strata-tmp-crashed"), "partial").expect("write");
    h.vault.evict(alice.id);

    let listed = ops::get_integrity(c).await.expect("integrity");
    let mut kinds: Vec<(String, Option<String>)> = listed
        .warnings
        .iter()
        .map(|w| (w.kind.clone(), w.path.clone()))
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        vec![
            ("id_assigned".to_owned(), Some("notes/Outside.md".to_owned())),
            ("out_of_band_edit".to_owned(), Some("notes/Edited.md".to_owned())),
            ("temp_file_removed".to_owned(), Some("notes/.strata-tmp-crashed".to_owned())),
            ("uncommitted_changes".to_owned(), None),
        ]
    );
    assert_eq!(h.log(alice.id)[0], "system: recovered changes");
    assert!(!dir.join("notes/.strata-tmp-crashed").exists());
    let note = ops::get_note(c, n.id).await.expect("note");
    assert!(note.content.ends_with("Edited outside.\n"), "{}", note.content);
    let found = ops::search(c, "Obsidian", None, None).await.expect("search");
    assert_eq!(
        found.hits.iter().map(|i| i.path.clone()).collect::<Vec<_>>(),
        vec!["notes/Outside.md".to_owned()]
    );
    // A clean vault verifies clean.
    let report = h.vault.verify(&h.db.scope(alice.id)).await.expect("verify");
    assert!(report.is_clean(), "{report:?}");
    h.finish().await;
}
