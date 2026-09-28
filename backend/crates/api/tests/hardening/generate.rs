//! Request generation from the OpenAPI schemas (PLAN §16.3 schema-driven fuzzer).
//!
//! - [`example`]: one deterministic, minimal, schema-valid value (required members only),
//!   with ULID fields filled from the caller's fixtures by field name.
//! - [`strategy`]: a `proptest` strategy of schema-valid values (optional members, mixed
//!   Arabic/Latin/markdown text, boundary integers, fixture or random ULIDs).
//! - [`Mutation`]: structural and wire-level corruptions of a valid body.
//!
//! Values are `rmpv::Value`s so `format: binary` becomes MessagePack `bin`, as on the wire.

use std::collections::BTreeMap;
use std::sync::Arc;

use proptest::prelude::*;
use proptest::sample::select;
use rmpv::Value as M;
use serde_json::Value as J;

/// Fixture IDs by kind (`note`, `entity`, `place`, ...), for ULID fields and path params.
#[derive(Debug, Clone, Default)]
pub struct Pools {
    /// Kind → IDs (never empty vectors).
    pub by_kind: BTreeMap<String, Vec<String>>,
}

impl Pools {
    /// Adds an ID of `kind`.
    pub fn add(&mut self, kind: &str, id: impl Into<String>) {
        self.by_kind
            .entry(kind.to_owned())
            .or_default()
            .push(id.into());
    }

    /// IDs of `kind` (empty if none).
    pub fn get(&self, kind: &str) -> &[String] {
        self.by_kind.get(kind).map_or(&[], Vec::as_slice)
    }

    /// Every ID of every kind.
    pub fn all(&self) -> Vec<String> {
        self.by_kind.values().flatten().cloned().collect()
    }
}

/// The fixture kind a ULID body/query field refers to, by field name.
pub fn field_kind(field: &str) -> &'static str {
    match field {
        "into_id" | "person_id" | "counterparty_id" | "entity" | "holder" | "entity_id" => "entity",
        "place_id" | "parent_id" | "place" => "place",
        "document_id" => "document",
        "device_id" => "device",
        "suggestion_id" => "suggestion",
        "task_id" => "task",
        // `src_id`, `dst_id`, `note_id`, `source_note_id`, `note`, `copy_of`, `other_id` and
        // any other ID field.
        _ => "note",
    }
}

/// Follows `$ref`s and folds `allOf` members into one object schema.
pub fn resolve(doc: &J, schema: &J) -> J {
    let mut s = schema.clone();
    for _ in 0..32 {
        let Some(r) = s.get("$ref").and_then(J::as_str) else {
            break;
        };
        s = doc
            .pointer(&r.replacen('#', "", 1))
            .cloned()
            .unwrap_or(J::Null);
    }
    if let Some(members) = s.get("allOf").and_then(J::as_array).cloned() {
        let mut props = serde_json::Map::new();
        let mut required = Vec::new();
        for m in &members {
            let m = resolve(doc, m);
            if let Some(p) = m.get("properties").and_then(J::as_object) {
                props.extend(p.clone());
            }
            if let Some(r) = m.get("required").and_then(J::as_array) {
                required.extend(r.clone());
            }
        }
        if let Some(p) = s.get("properties").and_then(J::as_object) {
            props.extend(p.clone());
        }
        if let Some(r) = s.get("required").and_then(J::as_array) {
            required.extend(r.clone());
        }
        return serde_json::json!({"type": "object", "properties": props, "required": required});
    }
    s
}

/// The non-null JSON types a schema allows.
fn types_of(s: &J) -> Vec<String> {
    match s.get("type") {
        Some(J::String(t)) => vec![t.clone()],
        Some(J::Array(ts)) => ts
            .iter()
            .filter_map(J::as_str)
            .filter(|t| *t != "null")
            .map(str::to_owned)
            .collect(),
        _ => {
            if s.get("properties").is_some() {
                vec!["object".to_owned()]
            } else {
                Vec::new()
            }
        }
    }
}

fn nullable(s: &J) -> bool {
    matches!(s.get("type"), Some(J::Array(ts)) if ts.iter().any(|t| t == "null"))
        || s.get("type").and_then(J::as_str) == Some("null")
}

/// JSON → MessagePack value (enum constants and the like).
pub fn json_to_mp(v: &J) -> M {
    match v {
        J::Null => M::Nil,
        J::Bool(b) => M::Boolean(*b),
        J::Number(n) => n.as_i64().map_or_else(
            || {
                n.as_u64()
                    .map_or_else(|| M::F64(n.as_f64().unwrap_or(0.0)), M::from)
            },
            M::from,
        ),
        J::String(s) => M::from(s.as_str()),
        J::Array(items) => M::Array(items.iter().map(json_to_mp).collect()),
        J::Object(map) => M::Map(
            map.iter()
                .map(|(k, v)| (M::from(k.as_str()), json_to_mp(v)))
                .collect(),
        ),
    }
}

/// A fixed, valid ULID used when no fixture applies.
pub const FALLBACK_ULID: &str = "01K00000000000000000000000";

/// One deterministic, minimal valid value. `ids(field)` supplies ULIDs by field name.
pub fn example(doc: &J, schema: &J, field: &str, ids: &dyn Fn(&str) -> Option<String>) -> M {
    let s = resolve(doc, schema);
    for key in ["oneOf", "anyOf"] {
        if let Some(members) = s.get(key).and_then(J::as_array) {
            let pick = members
                .iter()
                .find(|m| !nullable(&resolve(doc, m)) || types_of(&resolve(doc, m)).len() > 1)
                .or_else(|| members.first());
            return pick.map_or(M::Nil, |m| example(doc, m, field, ids));
        }
    }
    if let Some(values) = s.get("enum").and_then(J::as_array) {
        return values.first().map_or(M::Nil, json_to_mp);
    }
    let types = types_of(&s);
    let Some(t) = types.first() else {
        return M::Nil;
    };
    match t.as_str() {
        "object" => {
            let required: Vec<&str> = s
                .get("required")
                .and_then(J::as_array)
                .map(|r| r.iter().filter_map(J::as_str).collect())
                .unwrap_or_default();
            let mut entries = Vec::new();
            if let Some(props) = s.get("properties").and_then(J::as_object) {
                for (k, p) in props {
                    if required.contains(&k.as_str()) {
                        entries.push((M::from(k.as_str()), example(doc, p, k, ids)));
                    }
                }
            }
            M::Map(entries)
        }
        "array" => {
            let min = s.get("minItems").and_then(J::as_u64).unwrap_or(0);
            let item = s.get("items").cloned().unwrap_or(J::Null);
            M::Array((0..min).map(|_| example(doc, &item, field, ids)).collect())
        }
        "integer" => M::from(s.get("minimum").and_then(J::as_i64).unwrap_or(1).max(1)),
        "number" => M::F64(1.0),
        "boolean" => M::Boolean(false),
        "string" => match s.get("format").and_then(J::as_str) {
            Some("ulid") => M::from(ids(field).unwrap_or_else(|| FALLBACK_ULID.to_owned())),
            Some("date-time") => M::from("2026-09-27T12:00:00Z"),
            Some("date") => M::from("2026-09-27"),
            Some("binary") => M::Binary(Vec::new()),
            _ => M::from(example_text(field)),
        },
        _ => M::Nil,
    }
}

fn example_text(field: &str) -> String {
    if field.contains("path") {
        "notes/Example.md".to_owned()
    } else if field.contains("password") {
        "example-password-1".to_owned()
    } else if field == "timezone" {
        "UTC".to_owned()
    } else if field == "type" || field == "new_type" {
        "related".to_owned()
    } else {
        "Example text".to_owned()
    }
}

/// Mixed Arabic/Latin/markdown text, including hostile atoms.
pub fn text() -> BoxedStrategy<String> {
    let atom = prop_oneof![
        4 => "[a-zA-Z0-9 ]{0,12}",
        1 => "[\u{0621}-\u{064A} ]{1,10}",
        1 => select(vec![
            "واتانيا", "ووتانيا", "وطنية", "أحمد", "إيصال", "بكرة", "[[Watanya]]",
            "[[Plan#^b1]]", "![[image.png]]", "# Heading\n", "- [ ] Pay rent 📅 2026-10-01\n",
            "- [x] Done ✅ 2026-09-01\n", "^blk1", "#tag", "---\nid: x\n---\n", "related",
            "works-at", "part-of", "../", "..\\", "/etc/passwd", "notes/Fuzz.md", "", " ",
            "\u{0}", "\u{202e}", "\r\n", "ـــ", "é", "𝔘", "%2e%2e%2f", "C:\\x", "\u{feff}",
            "a\u{0301}", "ﻻ", "١٢٣", "{{x}}", "'; DROP TABLE notes; --", "<script>",
        ])
        .prop_map(str::to_owned),
        1 => Just("x".repeat(300)),
    ];
    prop::collection::vec(atom, 1..4)
        .prop_map(|v| v.concat())
        .boxed()
}

fn path_text() -> BoxedStrategy<String> {
    prop_oneof![
        3 => (
            select(vec!["notes/", "people/", "companies/", "inbox/", "", "maps/", "notes/sub/"]),
            "[A-Za-z\u{0627}-\u{064A}][A-Za-z0-9 \u{0627}-\u{064A}]{0,8}",
            select(vec![".md", ".md", "", ".canvas", ".txt"]),
        )
            .prop_map(|(d, w, e)| format!("{d}{w}{e}")),
        1 => text(),
    ]
    .boxed()
}

fn random_ulid() -> BoxedStrategy<String> {
    any::<u128>()
        .prop_map(|n| ulid::Ulid::from(n >> 2).to_string())
        .boxed()
}

/// ULIDs for `field`: fixtures of the field's kind, any fixture, or random.
pub fn ulid_for(pools: &Pools, kind: &str) -> BoxedStrategy<String> {
    let own = pools.get(kind).to_vec();
    let all = pools.all();
    match (own.is_empty(), all.is_empty()) {
        (false, _) => prop_oneof![
            6 => select(own),
            2 => select(all),
            2 => random_ulid(),
        ]
        .boxed(),
        (true, false) => prop_oneof![1 => select(all), 1 => random_ulid()].boxed(),
        (true, true) => random_ulid(),
    }
}

/// Context for [`strategy`].
#[derive(Debug)]
pub struct Ctx {
    /// The OpenAPI document.
    pub doc: J,
    /// Fixture IDs.
    pub pools: Pools,
}

/// Schema-valid values of `schema` (the member called `field`), nesting `depth`.
pub fn strategy(ctx: &Arc<Ctx>, schema: &J, field: &str, depth: u32) -> BoxedStrategy<M> {
    let s = resolve(&ctx.doc, schema);
    if depth > 6 {
        let v = example(&ctx.doc, &s, field, &|_| None);
        return Just(v).boxed();
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(members) = s.get(key).and_then(J::as_array) {
            let options: Vec<BoxedStrategy<M>> = members
                .iter()
                .map(|m| strategy(ctx, m, field, depth + 1))
                .collect();
            return proptest::strategy::Union::new(options).boxed();
        }
    }
    let base = non_null(ctx, &s, field, depth);
    if nullable(&s) {
        prop_oneof![1 => Just(M::Nil), 3 => base].boxed()
    } else {
        base
    }
}

fn non_null(ctx: &Arc<Ctx>, s: &J, field: &str, depth: u32) -> BoxedStrategy<M> {
    if let Some(values) = s.get("enum").and_then(J::as_array) {
        let values: Vec<M> = values.iter().map(json_to_mp).collect();
        return select(values).boxed();
    }
    let types = types_of(s);
    let Some(t) = types.first() else {
        return Just(M::Nil).boxed();
    };
    match t.as_str() {
        "object" => object(ctx, s, depth),
        "array" => {
            let item = s.get("items").cloned().unwrap_or(J::Null);
            let max = if depth > 3 { 2 } else { 4 };
            prop::collection::vec(strategy(ctx, &item, field, depth + 1), 0..max)
                .prop_map(M::Array)
                .boxed()
        }
        "integer" => {
            let lo = s.get("minimum").and_then(J::as_i64).unwrap_or(-5);
            let hi = s
                .get("maximum")
                .and_then(J::as_i64)
                .unwrap_or_else(|| lo.saturating_add(10_000));
            let near = lo.saturating_add(20).min(hi);
            prop_oneof![
                1 => Just(lo),
                1 => Just(hi),
                3 => lo..=near,
                2 => lo..=hi,
            ]
            .prop_map(M::from)
            .boxed()
        }
        "number" => prop_oneof![Just(0.0), Just(1.0), -1.0e6..1.0e6f64]
            .prop_map(M::F64)
            .boxed(),
        "boolean" => any::<bool>().prop_map(M::Boolean).boxed(),
        "string" => match s.get("format").and_then(J::as_str) {
            Some("ulid") => ulid_for(&ctx.pools, field_kind(field))
                .prop_map(M::from)
                .boxed(),
            Some("date-time") => (1_600_000_000i64..2_100_000_000)
                .prop_map(|t| {
                    M::from(
                        chrono::DateTime::from_timestamp(t, 0)
                            .unwrap_or_default()
                            .to_rfc3339(),
                    )
                })
                .boxed(),
            Some("date") => (2000i32..2040, 1u32..=12, 1u32..=28)
                .prop_map(|(y, m, d)| M::from(format!("{y:04}-{m:02}-{d:02}")))
                .boxed(),
            Some("binary") => prop::collection::vec(any::<u8>(), 0..64)
                .prop_map(M::Binary)
                .boxed(),
            _ if field.contains("path") => path_text().prop_map(M::from).boxed(),
            _ if field == "timezone" => select(vec!["UTC", "Africa/Cairo", "Mars/Olympus", ""])
                .prop_map(M::from)
                .boxed(),
            _ => text().prop_map(M::from).boxed(),
        },
        _ => Just(M::Nil).boxed(),
    }
}

fn object(ctx: &Arc<Ctx>, s: &J, depth: u32) -> BoxedStrategy<M> {
    let required: Vec<String> = s
        .get("required")
        .and_then(J::as_array)
        .map(|r| r.iter().filter_map(J::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let mut members: Vec<BoxedStrategy<Option<(M, M)>>> = Vec::new();
    if let Some(props) = s.get("properties").and_then(J::as_object) {
        for (k, p) in props {
            let key = M::from(k.as_str());
            let value = strategy(ctx, p, k, depth + 1);
            if required.contains(k) {
                members.push(value.prop_map(move |v| Some((key.clone(), v))).boxed());
            } else {
                members.push(
                    prop::option::weighted(0.5, value)
                        .prop_map(move |v| v.map(|v| (key.clone(), v)))
                        .boxed(),
                );
            }
        }
    }
    let extra = match s.get("additionalProperties") {
        Some(J::Object(_)) if s.get("properties").is_none() => {
            let value_schema = s["additionalProperties"].clone();
            prop::collection::vec(
                (
                    "[a-z_]{1,8}",
                    strategy(ctx, &value_schema, "value", depth + 1),
                ),
                0..3,
            )
            .prop_map(|kv| {
                kv.into_iter()
                    .map(|(k, v)| (M::from(k), v))
                    .collect::<Vec<_>>()
            })
            .boxed()
        }
        _ => Just(Vec::new()).boxed(),
    };
    (members, extra)
        .prop_map(|(members, extra)| {
            let mut entries: Vec<(M, M)> = members.into_iter().flatten().collect();
            for (k, v) in extra {
                if !entries.iter().any(|(ek, _)| *ek == k) {
                    entries.push((k, v));
                }
            }
            M::Map(entries)
        })
        .boxed()
}

/// A corruption applied to a valid body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    /// Unchanged.
    None,
    /// Remove one map entry (selector picks which).
    DropKey(u64),
    /// Replace one node by a value of another type.
    WrongType(u64),
    /// Add an undocumented member (must be ignored).
    UnknownField,
    /// Replace one string by 100 000 characters.
    HugeString(u64),
    /// Replace one node by 70 nested arrays.
    DeepNest(u64),
    /// Random bytes instead of MessagePack.
    RandomBytes(Vec<u8>),
    /// Empty body.
    Empty,
    /// Two bytes after the value.
    Trailing,
    /// Cut the encoding short.
    Truncated(u64),
    /// Declared `Content-Type: application/json`.
    JsonContentType,
    /// An array header declaring 4 294 967 295 elements and no elements.
    HugeArrayHeader,
    /// A map header declaring 4 294 967 295 entries.
    HugeMapHeader,
    /// A string header declaring 4 GiB.
    HugeStrHeader,
    /// A MessagePack extension value.
    Extension,
    /// `nil` as the whole body.
    Nil,
}

/// A mutation (half of the cases are unchanged bodies).
pub fn mutation() -> BoxedStrategy<Mutation> {
    prop_oneof![
        30 => Just(Mutation::None),
        4 => any::<u64>().prop_map(Mutation::DropKey),
        4 => any::<u64>().prop_map(Mutation::WrongType),
        2 => Just(Mutation::UnknownField),
        2 => any::<u64>().prop_map(Mutation::HugeString),
        1 => any::<u64>().prop_map(Mutation::DeepNest),
        1 => prop::collection::vec(any::<u8>(), 0..48).prop_map(Mutation::RandomBytes),
        1 => Just(Mutation::Empty),
        1 => Just(Mutation::Trailing),
        1 => any::<u64>().prop_map(Mutation::Truncated),
        1 => Just(Mutation::JsonContentType),
        1 => Just(Mutation::HugeArrayHeader),
        1 => Just(Mutation::HugeMapHeader),
        1 => Just(Mutation::HugeStrHeader),
        1 => Just(Mutation::Extension),
        1 => Just(Mutation::Nil),
    ]
    .boxed()
}

/// Every node of `v` as a path of child indices (root first).
fn node_paths(v: &M, prefix: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
    out.push(prefix.clone());
    match v {
        M::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                prefix.push(i);
                node_paths(item, prefix, out);
                prefix.pop();
            }
        }
        M::Map(entries) => {
            for (i, (_, value)) in entries.iter().enumerate() {
                prefix.push(i);
                node_paths(value, prefix, out);
                prefix.pop();
            }
        }
        _ => {}
    }
}

fn node_mut<'a>(v: &'a mut M, path: &[usize]) -> &'a mut M {
    let Some((first, rest)) = path.split_first() else {
        return v;
    };
    match v {
        M::Array(items) => node_mut(&mut items[*first], rest),
        M::Map(entries) => node_mut(&mut entries[*first].1, rest),
        _ => v,
    }
}

fn pick(v: &M, selector: u64, filter: impl Fn(&M) -> bool) -> Option<Vec<usize>> {
    let mut all = Vec::new();
    node_paths(v, &mut Vec::new(), &mut all);
    let mut root = v.clone();
    let candidates: Vec<Vec<usize>> = all
        .into_iter()
        .filter(|p| filter(node_mut(&mut root, p)))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let n = u64::try_from(candidates.len()).unwrap_or(1);
    let i = usize::try_from(selector % n).unwrap_or(0);
    Some(candidates[i].clone())
}

fn encode(v: &M) -> Vec<u8> {
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, v).expect("encode");
    out
}

/// The content type and bytes of `value` after `mutation`.
pub fn apply(value: &M, mutation: &Mutation) -> (String, Vec<u8>) {
    let msgpack = strata_api::wire::MSGPACK.to_owned();
    let mut v = value.clone();
    let bytes = match mutation {
        Mutation::None => encode(&v),
        Mutation::DropKey(sel) => {
            if let Some(p) = pick(&v, *sel, |n| matches!(n, M::Map(e) if !e.is_empty()))
                && let M::Map(entries) = node_mut(&mut v, &p)
            {
                let i =
                    usize::try_from(*sel % u64::try_from(entries.len()).unwrap_or(1)).unwrap_or(0);
                entries.remove(i);
            }
            encode(&v)
        }
        Mutation::WrongType(sel) => {
            if let Some(p) = pick(&v, *sel, |_| true) {
                let node = node_mut(&mut v, &p);
                *node = match node {
                    M::String(_) => M::from(-7),
                    M::Integer(_) | M::F64(_) | M::F32(_) => M::from("seven"),
                    M::Boolean(_) => M::from("true"),
                    M::Array(_) => M::from(true),
                    M::Map(_) => M::Array(vec![M::from(1)]),
                    _ => M::Map(vec![]),
                };
            }
            encode(&v)
        }
        Mutation::UnknownField => {
            if let M::Map(entries) = &mut v {
                entries.push((M::from("zz_unknown_member"), M::from(1)));
            }
            encode(&v)
        }
        Mutation::HugeString(sel) => {
            if let Some(p) = pick(&v, *sel, |n| matches!(n, M::String(_))) {
                *node_mut(&mut v, &p) = M::from("ق".repeat(50_000));
            }
            encode(&v)
        }
        Mutation::DeepNest(sel) => {
            if let Some(p) = pick(&v, *sel, |_| true) {
                let mut deep = M::from(1);
                for _ in 0..70 {
                    deep = M::Array(vec![deep]);
                }
                *node_mut(&mut v, &p) = deep;
            }
            encode(&v)
        }
        Mutation::RandomBytes(b) => b.clone(),
        Mutation::Empty => Vec::new(),
        Mutation::Trailing => {
            let mut b = encode(&v);
            b.extend_from_slice(&[0x01, 0x02]);
            b
        }
        Mutation::Truncated(sel) => {
            let b = encode(&v);
            let n = u64::try_from(b.len()).unwrap_or(1).max(1);
            b[..usize::try_from(sel % n).unwrap_or(0)].to_vec()
        }
        Mutation::JsonContentType => {
            return ("application/json".to_owned(), b"{\"a\":1}".to_vec());
        }
        Mutation::HugeArrayHeader => vec![0xdd, 0xff, 0xff, 0xff, 0xff, 0x01],
        Mutation::HugeMapHeader => vec![0xdf, 0xff, 0xff, 0xff, 0xff, 0xa1, b'a', 0x01],
        Mutation::HugeStrHeader => vec![0xdb, 0xff, 0xff, 0xff, 0xff, b'a', b'b'],
        Mutation::Extension => vec![0xd4, 0x01, 0x00],
        Mutation::Nil => vec![0xc0],
    };
    (msgpack, bytes)
}
