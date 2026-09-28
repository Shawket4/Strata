//! Text-normalisation benchmarks on large mixed Arabic/English input (PLAN §16.7): the search
//! normaliser over 1 MiB, and the duplicate/alias keys and trigram similarity over 10 000
//! titles, as the indexer and the offline client run them.
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used)] // bench harness

use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};

const WORDS: [&str; 20] = [
    "Pricing",
    "INVOICE",
    "contract",
    "café",
    "Straße",
    "ﬁnance",
    "مَدْرَسَةٌ",
    "إيصال",
    "أحمد",
    "آمنة",
    "مكتبة",
    "ـــخزنة",
    "فاتورة",
    "مستشفى",
    "ووتانيا",
    "وطنية",
    "١٢٣",
    "Ahmed",
    "ETA",
    "Watanya's",
];

fn mixed(len: usize) -> String {
    let mut out = String::new();
    let mut i = 0;
    while out.len() < len {
        out.push_str(WORDS[(i * 7 + i / 3) % WORDS.len()]);
        out.push(if i % 11 == 0 { '\n' } else { ' ' });
        i += 1;
    }
    out
}

fn titles(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            format!(
                "{} {} {i}",
                WORDS[i % WORDS.len()],
                WORDS[(i * 3 + 1) % WORDS.len()]
            )
        })
        .collect()
}

fn benches(c: &mut Criterion) {
    let text = mixed(1024 * 1024);
    let mut group = c.benchmark_group("text_normalize_1mib");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Bytes(text.len() as u64));
    group.bench_function("normalize_for_search", |b| {
        b.iter(|| black_box(text_normalize::normalize_for_search(black_box(&text))));
    });
    group.bench_function("transliteration_key", |b| {
        b.iter(|| black_box(text_normalize::transliteration_key(black_box(&text))));
    });
    group.finish();

    let titles = titles(10_000);
    let mut group = c.benchmark_group("text_normalize_10k_titles");
    group.sample_size(20);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Elements(titles.len() as u64));
    group.bench_function("dedupe_key", |b| {
        b.iter(|| {
            for t in &titles {
                black_box(text_normalize::dedupe_key(t));
            }
        });
    });
    group.bench_function("trigrams", |b| {
        b.iter(|| {
            for t in &titles {
                black_box(text_normalize::trigrams(t));
            }
        });
    });
    group.bench_function("trigram_similarity_vs_one", |b| {
        let probe = "Watanya's ETA invoice";
        b.iter(|| {
            for t in &titles {
                black_box(text_normalize::trigram_similarity(probe, t));
            }
        });
    });
    group.finish();
}

criterion_group!(normalize_benches, benches);
criterion_main!(normalize_benches);
