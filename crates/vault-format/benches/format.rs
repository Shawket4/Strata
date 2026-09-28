//! Vault-format parse/serialise benchmarks on large inputs (PLAN §16.7): one 1 MiB note
//! (frontmatter, mixed Arabic/English prose, links, tags, block IDs, task lines) and a batch of
//! 1 000 ordinary notes, as the indexer and the client core see them.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)] // bench harness

use std::fmt::Write as _;
use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use vault_format::Document;

const EN: [&str; 12] = [
    "pricing",
    "invoice",
    "contract",
    "meeting",
    "customer",
    "delivery",
    "budget",
    "supplier",
    "renewal",
    "shipment",
    "warehouse",
    "payment",
];
const AR: [&str; 8] = [
    "فاتورة",
    "عقد",
    "اجتماع",
    "عميل",
    "توصيل",
    "ميزانية",
    "مورد",
    "تجديد",
];

fn frontmatter(i: usize) -> String {
    format!(
        "---\nid: 01K5Z{i:021}\ntitle: Note {i}\naliases: [ملاحظة {i}]\ntags: [pos, finance]\ncreated: 2026-09-27T14:32:00+03:00\nupdated: 2026-09-27T15:10:00+03:00\nrelated: [\"[[Note {}]]\", \"[[Note {}]]\"]\npeople: [\"[[Ahmed Samir]]\"]\ncustom-key: kept as is\n---\n",
        i + 1,
        i + 2
    )
}

fn paragraph(i: usize) -> String {
    let mut words = Vec::new();
    for w in 0..24 {
        words.push(if (i + w).is_multiple_of(3) {
            AR[(i + w) % AR.len()]
        } else {
            EN[(i + w) % EN.len()]
        });
    }
    let mut p = words.join(" ");
    let _ = write!(
        p,
        " [[Note {}#Heading|alias]] #tag{} ![[image{i}.png]] ^b{i}\n\n",
        i % 97,
        i % 13
    );
    if i.is_multiple_of(4) {
        let _ = write!(
            p,
            "- [ ] {} 📅 2026-10-{:02} 🔁 every month on the 1st (@2026-10-01 09:00) ^t-01k5z{i:021}\n\n",
            EN[i % EN.len()],
            1 + i % 28
        );
    }
    if i.is_multiple_of(10) {
        let _ = write!(p, "## Heading {i}\n\n");
    }
    p
}

fn large_note() -> String {
    let mut text = frontmatter(0);
    let mut i = 0;
    while text.len() < 1024 * 1024 {
        text.push_str(&paragraph(i));
        i += 1;
    }
    text
}

fn small_notes(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let mut t = frontmatter(i);
            for p in 0..4 {
                t.push_str(&paragraph(i * 4 + p));
            }
            t
        })
        .collect()
}

fn benches(c: &mut Criterion) {
    let large = large_note();
    let mut group = c.benchmark_group("vault_format_1mib_note");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Bytes(large.len() as u64));
    group.bench_function("parse", |b| {
        b.iter(|| black_box(Document::parse(black_box(&large))));
    });
    let doc = Document::parse(&large);
    assert_eq!(doc.render(), large, "byte-exact round trip");
    group.bench_function("render", |b| b.iter(|| black_box(doc.render())));
    group.bench_function("render_canonical", |b| {
        b.iter(|| black_box(doc.render_canonical()));
    });
    group.bench_function("analyze_body", |b| b.iter(|| black_box(doc.analyze_body())));
    group.bench_function("extract_tasks", |b| {
        b.iter(|| black_box(vault_format::tasks::extract_tasks(black_box(doc.body()))));
    });
    group.bench_function("parse_render_round_trip", |b| {
        b.iter(|| black_box(Document::parse(black_box(&large)).render()));
    });
    group.finish();

    let notes = small_notes(1_000);
    let bytes: usize = notes.iter().map(String::len).sum();
    let mut group = c.benchmark_group("vault_format_1000_notes");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Bytes(bytes as u64));
    group.bench_function("parse_analyze", |b| {
        b.iter(|| {
            for n in &notes {
                let d = Document::parse(n);
                black_box(d.analyze_body());
            }
        });
    });
    group.bench_function("frontmatter_edit_render", |b| {
        b.iter_batched(
            || notes.iter().map(|n| Document::parse(n)).collect::<Vec<_>>(),
            |mut docs| {
                for d in &mut docs {
                    d.set_body(format!("{}\nappended line\n", d.body()));
                    black_box(d.render_canonical());
                }
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

criterion_group!(format_benches, benches);
criterion_main!(format_benches);
