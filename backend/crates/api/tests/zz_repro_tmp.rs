#![allow(clippy::expect_used)]
mod hardening;
use std::io::{Cursor, Write};
use hardening::http::Req;
use hardening::{H, Options};

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, b) in entries { z.start_file(*n, zip::write::SimpleFileOptions::default()).expect("e"); z.write_all(b).expect("w"); }
    z.finish().expect("z").into_inner()
}

#[tokio::test]
async fn repro() {
    let h = H::with(Options { config: Box::new(hardening::generous_limits), ..Options::default() }).await;
    let u = h.user("fuzzer").await;
    let fx = hardening::populate(&h, &u, "t").await;
    let _ = fx;
    let cases: Vec<(&str, Vec<(&str, &[u8])>)> = vec![
        ("first", vec![("notes/U.md", b"one")]),
        ("same again", vec![("notes/U.md", b"one")]),
        ("changed", vec![("notes/U.md", b"two")]),
        ("case clash", vec![("notes/u.md", b"three")]),
        ("existing note path", vec![("notes/Plan.md", b"replaced")]),
        ("same title other folder", vec![("other/U.md", b"four")]),
        ("two same", vec![("notes/V.md", b"a"), ("notes/v.md", b"b")]),
        ("people clash", vec![("notes/Watanya.md", b"x")]),
        ("trash path", vec![(".trash/notes/Old.md", b"x")]),
        ("meta", vec![(".meta/notes/x.json", b"{")]),
        ("task home", vec![("tasks/Tasks.md", b"- [ ] x\n")]),
    ];
    for (label, e) in cases {
        let r = h.send(None, &Req::new("POST", "/api/v1/import").token(&u.token).body("application/zip", zip_of(&e))).await;
        eprintln!("{label}: {} {:?} {}", r.status, r.problem_type(), String::from_utf8_lossy(&r.body).chars().take(150).collect::<String>());
    }
}
