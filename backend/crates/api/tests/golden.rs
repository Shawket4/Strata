//! Byte-exact wire goldens (PLAN §7.7, §16.3).
//!
//! Each fixture in `tests/golden/<name>.hex` is the exact MessagePack encoding of a
//! representative payload (hex, 16 bytes per line). The test asserts that encoding produces the
//! fixture, that decoding the fixture gives the value back, and that `docs/WIRE_FORMAT.md`
//! quotes the fixture verbatim. Regenerate after an intended change with
//! `STRATA_UPDATE_GOLDEN=1 cargo test -p strata-api --test golden`, then review the diff.

use std::fmt::Debug;
use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use strata_api::health::{Health, HealthStatus};
use strata_api::testing::demo::{CreateWidget, DemoTick, Shape, Widget};
use strata_api::wire::ws::Frame;
use strata_api::wire::{
    Binary, DecodeError, DecodeLimits, DuplicateCandidate, MatchLevel, Problem, ProblemType,
    Violation, decode, encode,
};
use ulid::Ulid;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/{name}.hex"))
}

fn to_hex_lines(bytes: &[u8]) -> String {
    bytes
        .chunks(16)
        .map(|line| {
            line.iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn parse_hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).expect("hex byte"))
        .collect()
}

fn updating() -> bool {
    std::env::var_os("STRATA_UPDATE_GOLDEN").is_some()
}

fn wire_format_doc() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/WIRE_FORMAT.md");
    std::fs::read_to_string(path).unwrap_or_default()
}

fn golden<T: Serialize + DeserializeOwned + PartialEq + Debug>(name: &str, value: &T) {
    let bytes = encode(value).expect("encodes");
    let path = fixture_path(name);
    if updating() {
        std::fs::write(&path, to_hex_lines(&bytes)).expect("writes fixture");
    }
    let fixture = std::fs::read_to_string(&path).expect("fixture exists");
    assert_eq!(to_hex_lines(&bytes), fixture, "encoding of {name}");
    let decoded: T = decode(&parse_hex(&fixture), &DecodeLimits::default()).expect("decodes");
    assert_eq!(&decoded, value, "decoding of {name}");
    assert!(
        updating() || wire_format_doc().contains(fixture.trim_end()),
        "docs/WIRE_FORMAT.md must quote tests/golden/{name}.hex"
    );
}

fn frame_golden<P: Serialize + DeserializeOwned + PartialEq + Debug>(
    name: &str,
    frame: &Frame<P>,
) {
    let bytes = frame.encode().expect("encodes");
    let path = fixture_path(name);
    if updating() {
        std::fs::write(&path, to_hex_lines(&bytes)).expect("writes fixture");
    }
    let fixture = std::fs::read_to_string(&path).expect("fixture exists");
    assert_eq!(to_hex_lines(&bytes), fixture, "encoding of {name}");
    assert_eq!(
        &Frame::<P>::decode(&parse_hex(&fixture), &DecodeLimits::default()).expect("decodes"),
        frame
    );
    assert!(
        updating() || wire_format_doc().contains(fixture.trim_end()),
        "docs/WIRE_FORMAT.md must quote tests/golden/{name}.hex"
    );
}

fn at(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).single().expect("valid timestamp")
}

fn ulid() -> Ulid {
    "01J8ZK3M4X7Q9V2B6C8D0E1F2G".parse().expect("valid ULID")
}

#[test]
fn health() {
    golden(
        "health",
        &Health {
            status: HealthStatus::Ok,
        },
    );
}

#[test]
fn tagged_enum_variants() {
    golden("shape_circle", &Shape::Circle { radius: 1.5 });
    golden(
        "shape_rect",
        &Shape::Rect {
            width: 3,
            height: 300,
        },
    );
}

#[test]
fn create_request_with_bytes_timestamp_and_absent_optional() {
    golden(
        "create_widget",
        &CreateWidget {
            name: "وثيقة".to_owned(),
            shape: Shape::Circle { radius: 1.5 },
            tags: vec!["a".to_owned()],
            blob: Binary(vec![0xde, 0xad, 0xbe, 0xef]),
            due: Some(at(1_790_000_000)),
            force: false,
        },
    );
}

#[test]
fn response_with_ulid() {
    golden(
        "widget",
        &Widget {
            id: ulid(),
            name: "w".to_owned(),
            shape: Shape::Rect {
                width: 1,
                height: 2,
            },
            tags: vec![],
            blob: Binary(vec![]),
            due: None,
            version: "v1.1".to_owned(),
            created: at(1_790_000_001),
        },
    );
}

#[test]
fn problems() {
    golden(
        "problem_invalid_body",
        &Problem::invalid_body(&DecodeError::Structure(Violation::TrailingBytes { count: 2 })),
    );
    golden(
        "problem_duplicate_candidates",
        &Problem::duplicate_candidates(vec![DuplicateCandidate {
            id: ulid(),
            kind: "task".to_owned(),
            title: "ETA invoice".to_owned(),
            snippet: None,
            match_level: MatchLevel::Near,
            score: 0.5,
        }]),
    );
    golden("problem_version_conflict", &Problem::version_conflict("h1"));
    golden("problem_not_found", &Problem::new(ProblemType::NotFound));
}

#[test]
fn stream_frames() {
    frame_golden(
        "frame_data",
        &Frame::Data {
            seq: 42,
            payload: DemoTick {
                n: 42,
                label: "t".to_owned(),
            },
        },
    );
    frame_golden(
        "frame_error",
        &Frame::<DemoTick>::Error {
            seq: 42,
            problem: Problem::new(ProblemType::Internal),
        },
    );
    frame_golden("frame_reset", &Frame::<DemoTick>::Reset { seq: 300 });
    frame_golden("frame_end", &Frame::<DemoTick>::End { seq: 42 });
}
