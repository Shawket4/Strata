//! Every documented operation of the production contract, read from the OpenAPI document
//! (so operations added later are swept automatically).

use std::collections::BTreeSet;

use serde_json::Value;
use strata_api::contract::Contract;

/// Where a parameter goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Loc {
    /// A `{name}` path segment.
    Path,
    /// A query-string parameter.
    Query,
    /// A request header.
    Header,
}

/// One documented parameter.
#[derive(Debug, Clone)]
pub struct Param {
    /// Its name.
    pub name: String,
    /// Where it goes.
    pub loc: Loc,
    /// Required?
    pub required: bool,
    /// Its schema.
    pub schema: Value,
}

/// The request body of an operation.
#[derive(Debug, Clone)]
pub enum Body {
    /// `application/vnd.msgpack` with this schema.
    MsgPack(Value),
    /// `application/zip`.
    Zip,
}

/// One documented operation.
#[derive(Debug, Clone)]
pub struct Op {
    /// `operationId`.
    pub id: String,
    /// Upper-case method.
    pub method: String,
    /// Path template including `/api/v1`.
    pub path: String,
    /// Parameters in document order.
    pub params: Vec<Param>,
    /// Request body.
    pub body: Option<Body>,
    /// A WebSocket stream (`x-strata-stream`).
    pub stream: bool,
    /// Needs a bearer token (documents `401`).
    pub secured: bool,
    /// Documented statuses.
    pub statuses: BTreeSet<u16>,
}

impl Op {
    /// The path parameters.
    pub fn path_params(&self) -> impl Iterator<Item = &Param> {
        self.params.iter().filter(|p| p.loc == Loc::Path)
    }

    /// The path with every `{name}` replaced by `value(name)` (inserted verbatim).
    pub fn fill(&self, mut value: impl FnMut(&str) -> String) -> String {
        let mut out = String::new();
        let mut rest = self.path.as_str();
        while let Some(start) = rest.find('{') {
            out.push_str(&rest[..start]);
            let end = rest[start..].find('}').map_or(rest.len(), |e| start + e);
            out.push_str(&value(&rest[start + 1..end]));
            rest = &rest[(end + 1).min(rest.len())..];
        }
        out.push_str(rest);
        out
    }

    /// True if the operation belongs to the admin API.
    pub fn is_admin(&self) -> bool {
        self.path.starts_with("/api/v1/admin/")
    }
}

/// Every operation, sorted by operation id.
pub fn operations(contract: &Contract) -> Vec<Op> {
    let doc = contract.document();
    let mut out = Vec::new();
    let Some(paths) = doc.get("paths").and_then(Value::as_object) else {
        return out;
    };
    for (path, item) in paths {
        let Some(item) = item.as_object() else {
            continue;
        };
        for (method, op) in item {
            let Some(id) = op.get("operationId").and_then(Value::as_str) else {
                continue;
            };
            let params = op
                .get("parameters")
                .and_then(Value::as_array)
                .map(|ps| {
                    ps.iter()
                        .map(|p| {
                            let p = deref(doc, p);
                            Param {
                                name: p["name"].as_str().unwrap_or_default().to_owned(),
                                loc: match p["in"].as_str() {
                                    Some("path") => Loc::Path,
                                    Some("header") => Loc::Header,
                                    _ => Loc::Query,
                                },
                                required: p["required"].as_bool().unwrap_or(false),
                                schema: p.get("schema").cloned().unwrap_or(Value::Null),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            let body = op.pointer("/requestBody/content").and_then(|c| {
                if let Some(m) = c.get(strata_api::wire::MSGPACK) {
                    Some(Body::MsgPack(
                        m.get("schema").cloned().unwrap_or(Value::Null),
                    ))
                } else if c.get(strata_api::wire::ZIP).is_some() {
                    Some(Body::Zip)
                } else {
                    None
                }
            });
            let statuses: BTreeSet<u16> = op["responses"]
                .as_object()
                .map(|r| r.keys().filter_map(|k| k.parse().ok()).collect())
                .unwrap_or_default();
            out.push(Op {
                id: id.to_owned(),
                method: method.to_ascii_uppercase(),
                path: path.clone(),
                params,
                body,
                stream: op.get(strata_api::wire::ws::STREAM_EXTENSION).is_some(),
                secured: statuses.contains(&401),
                statuses,
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Resolves a local `$ref` (one level).
pub fn deref<'a>(doc: &'a Value, v: &'a Value) -> &'a Value {
    match v.get("$ref").and_then(Value::as_str) {
        Some(r) => doc.pointer(&r.replacen('#', "", 1)).unwrap_or(&Value::Null),
        None => v,
    }
}
