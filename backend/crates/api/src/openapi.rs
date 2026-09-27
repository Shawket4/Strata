//! The OpenAPI 3.1 contract (L13, §7.6), written to `api/openapi.json`.
//!
//! Handlers are annotated with `#[utoipa::path]` using paths relative to `/api/v1` and listed
//! in [`ApiDoc`]. [`build`] then applies the wire conventions to utoipa's output, so every
//! operation is described the same way:
//!
//! - paths are prefixed with `/api/v1`;
//! - `application/json` (utoipa's default) becomes `application/msgpack` for request bodies
//!   and 2xx responses and `application/problem+msgpack` for error responses;
//! - standard problem responses are added: `406` and `500` everywhere, `401` on secured
//!   operations, `404` when there are path parameters, `422 invalid_parameter` when there are
//!   query/header parameters, `413`/`415`/`422 invalid_body` when there is a request body;
//! - bearer security is the default; operations opt out with `security(())`;
//! - `oneOf` schemas whose members share a single-valued required property (tagged enums, see
//!   `docs/WIRE_FORMAT.md`) get a `discriminator`;
//! - stream endpoints ([`StreamOperation`]) are described with the `x-strata-stream` extension
//!   and their frame schemas are registered as named components.
//!
//! [`lint`] checks the result; the output is canonical (sorted keys) pretty JSON.

use std::io;
use std::path::Path;

use serde_json::{Map, Value, json};
use utoipa::OpenApi;

use crate::app::API_PREFIX;
use crate::wire::ws::{FrameKind, RESUME_PARAM, STREAM_EXTENSION};
use crate::wire::{
    Binary, DuplicateCandidate, MSGPACK, MatchLevel, PROBLEM_MSGPACK, Problem, ProblemFieldError,
    ZIP,
};

/// Every production operation and shared component. Endpoint authors add their handlers to
/// `paths(...)` (and schemas only reachable through streams to `components(...)`).
#[derive(Debug, OpenApi)]
#[openapi(
    paths(crate::health::health),
    components(schemas(
        Problem,
        ProblemFieldError,
        DuplicateCandidate,
        MatchLevel,
        Binary,
        FrameKind
    )),
    tags((name = "system", description = "Service status."))
)]
pub struct ApiDoc;

/// A WebSocket stream endpoint (D24), documented with the `x-strata-stream` extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamOperation {
    /// Path relative to `/api/v1`, e.g. `/events`.
    pub path: &'static str,
    /// Operation id (becomes the generated client function name).
    pub operation_id: &'static str,
    /// Tag.
    pub tag: &'static str,
    /// One-line summary.
    pub summary: &'static str,
    /// Component schema name of the `data` frame payload.
    pub payload: &'static str,
    /// Whether a bearer token is required.
    pub secured: bool,
}

/// Production stream endpoints.
pub const STREAMS: &[StreamOperation] = &[];

/// The production contract.
pub fn document() -> Value {
    build(&ApiDoc::openapi(), STREAMS)
}

/// Canonical pretty JSON of `doc`, newline-terminated.
pub fn to_pretty_json(doc: &Value) -> String {
    let mut out = serde_json::to_string_pretty(&canonical(doc)).unwrap_or_default();
    out.push('\n');
    out
}

/// Writes the production contract to `path` (normally `api/openapi.json`).
pub fn write(path: impl AsRef<Path>) -> io::Result<()> {
    std::fs::write(path, to_pretty_json(&document()))
}

/// Applies the conventions described in the module docs to a utoipa document.
pub fn build(api: &utoipa::openapi::OpenApi, streams: &[StreamOperation]) -> Value {
    let mut doc = serde_json::to_value(api).unwrap_or_else(|_| json!({}));
    doc["openapi"] = json!("3.1.0");
    doc["info"] = json!({
        "title": "Strata API",
        "version": "1",
        "description": "Strata HTTP API. Every body is MessagePack (application/msgpack); \
                        errors are RFC 7807 problem details (application/problem+msgpack). \
                        See docs/WIRE_FORMAT.md.",
    });
    doc["security"] = json!([{ "bearer": [] }]);

    let paths = doc
        .get("paths")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut prefixed = Map::new();
    for (path, item) in paths {
        prefixed.insert(format!("{API_PREFIX}{path}"), item);
    }
    doc["paths"] = Value::Object(prefixed);

    if !doc["components"].is_object() {
        doc["components"] = json!({});
    }
    if !doc["components"]["schemas"].is_object() {
        doc["components"]["schemas"] = json!({});
    }
    doc["components"]["securitySchemes"] = json!({
        "bearer": {
            "type": "http",
            "scheme": "bearer",
            "description": "Short-lived access token (PLAN §8).",
        }
    });
    doc["components"]["responses"] = standard_responses();
    register_frame_schemas(&mut doc["components"]["schemas"], streams);

    if let Some(paths) = doc["paths"].as_object_mut() {
        for item in paths.values_mut() {
            let Some(item) = item.as_object_mut() else {
                continue;
            };
            for (method, op) in item.iter_mut() {
                if HTTP_METHODS.contains(&method.as_str()) {
                    apply_operation_conventions(op);
                }
            }
        }
    }
    for stream in streams {
        let path = format!("{API_PREFIX}{}", stream.path);
        doc["paths"][path.as_str()] = json!({ "get": stream_operation(stream) });
    }

    let schemas = doc["components"]["schemas"].clone();
    add_discriminators(&mut doc, &schemas);
    canonical(&doc)
}

const HTTP_METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

fn problem_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": { PROBLEM_MSGPACK: { "schema": { "$ref": "#/components/schemas/Problem" } } },
    })
}

fn standard_responses() -> Value {
    json!({
        "NotAcceptable": problem_response("`not_acceptable`: `Accept` excludes application/msgpack."),
        "UnsupportedMediaType": problem_response("`unsupported_media_type`: the body is not application/msgpack."),
        "PayloadTooLarge": problem_response("`payload_too_large`: the body exceeds the route's limit."),
        "InvalidBody": problem_response("`invalid_body`: the body failed decoding or validation."),
        "InvalidParameter": problem_response("`invalid_parameter`: a query or header parameter is invalid."),
        "Unauthorized": problem_response("`unauthorized`: missing, invalid or expired access token."),
        "NotFound": problem_response("`not_found`: no such resource in the caller's scope."),
        "Internal": problem_response("`internal`: unexpected server error."),
    })
}

fn response_ref(name: &str) -> Value {
    json!({ "$ref": format!("#/components/responses/{name}") })
}

fn apply_operation_conventions(op: &mut Value) {
    // Media types.
    if let Some(content) = op.pointer_mut("/requestBody/content") {
        rename_json_content(content, MSGPACK);
    }
    if let Some(responses) = op.get_mut("responses").and_then(Value::as_object_mut) {
        for (status, response) in responses.iter_mut() {
            let target = if status.starts_with('2') || status.starts_with('1') {
                MSGPACK
            } else {
                PROBLEM_MSGPACK
            };
            if let Some(content) = response.get_mut("content") {
                rename_json_content(content, target);
            }
        }
    }

    let has_body = op.get("requestBody").is_some();
    let params: Vec<String> = op
        .get("parameters")
        .and_then(Value::as_array)
        .map(|ps| {
            ps.iter()
                .filter_map(|p| p.get("in").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let public = op
        .get("security")
        .and_then(Value::as_array)
        .is_some_and(|reqs| {
            reqs.iter()
                .any(|r| r.as_object().is_some_and(Map::is_empty))
        });

    let mut add = |status: &str, name: &str| {
        if op["responses"].get(status).is_none() {
            op["responses"][status] = response_ref(name);
        }
    };
    add("406", "NotAcceptable");
    add("500", "Internal");
    if !public {
        add("401", "Unauthorized");
    }
    if params.iter().any(|p| p == "path") {
        add("404", "NotFound");
    }
    if has_body {
        add("413", "PayloadTooLarge");
        add("415", "UnsupportedMediaType");
        add("422", "InvalidBody");
    } else if params.iter().any(|p| p == "query" || p == "header") {
        add("422", "InvalidParameter");
    }
}

fn rename_json_content(content: &mut Value, target: &str) {
    let Some(map) = content.as_object_mut() else {
        return;
    };
    if let Some(media) = map.remove("application/json") {
        map.insert(target.to_owned(), media);
    }
}

fn frame_schema(kind: &[&str], payload: &Value, description: &str) -> Value {
    json!({
        "type": "object",
        "description": description,
        "required": ["v", "kind", "seq", "payload"],
        "properties": {
            "v": { "type": "integer", "format": "int32", "minimum": 1, "maximum": 65535,
                   "description": "Envelope version." },
            "kind": { "type": "string", "enum": kind },
            "seq": { "type": "integer", "format": "int64", "minimum": 0,
                     "description": "Sequence number; see docs/WIRE_FORMAT.md." },
            "payload": payload,
        },
    })
}

fn register_frame_schemas(schemas: &mut Value, streams: &[StreamOperation]) {
    schemas["ErrorFrame"] = frame_schema(
        &["error"],
        &json!({ "$ref": "#/components/schemas/Problem" }),
        "Terminal stream failure.",
    );
    schemas["ControlFrame"] = frame_schema(
        &["reset", "end"],
        &json!({ "type": "null" }),
        "`reset`: refetch state, then continue after `seq`. `end`: the stream finished.",
    );
    for stream in streams {
        let data = format!("{}DataFrame", stream.payload);
        schemas[data.as_str()] = frame_schema(
            &["data"],
            &json!({ "$ref": format!("#/components/schemas/{}", stream.payload) }),
            "A stream item.",
        );
        schemas[format!("{}Frame", stream.payload).as_str()] = json!({
            "description": format!("A frame of a `{}` stream (binary WebSocket message).", stream.payload),
            "oneOf": [
                { "$ref": format!("#/components/schemas/{data}") },
                { "$ref": "#/components/schemas/ErrorFrame" },
                { "$ref": "#/components/schemas/ControlFrame" },
            ],
            "discriminator": {
                "propertyName": "kind",
                "mapping": {
                    "data": format!("#/components/schemas/{data}"),
                    "error": "#/components/schemas/ErrorFrame",
                    "reset": "#/components/schemas/ControlFrame",
                    "end": "#/components/schemas/ControlFrame",
                },
            },
        });
    }
}

fn stream_operation(stream: &StreamOperation) -> Value {
    let mut responses = json!({
        "101": {
            "description": format!(
                "WebSocket upgrade; the server then sends binary `{}Frame` messages.",
                stream.payload
            ),
        },
        "406": response_ref("NotAcceptable"),
        "422": response_ref("InvalidParameter"),
        "500": response_ref("Internal"),
    });
    let mut op = json!({
        "operationId": stream.operation_id,
        "tags": [stream.tag],
        "summary": stream.summary,
        "parameters": [{
            "name": RESUME_PARAM,
            "in": "query",
            "required": false,
            "description": "Last seq received; frames after it are replayed (or `reset` is sent).",
            "schema": { "type": "integer", "format": "int64", "minimum": 0 },
        }],
        STREAM_EXTENSION: {
            "payload": stream.payload,
            "frame": format!("{}Frame", stream.payload),
        },
    });
    if stream.secured {
        responses["401"] = response_ref("Unauthorized");
    } else {
        op["security"] = json!([{}]);
    }
    op["responses"] = responses;
    op
}

/// Adds `discriminator` to `oneOf` schemas whose members all carry a required property with a
/// single string value (internally tagged enums).
fn add_discriminators(value: &mut Value, schemas: &Value) {
    match value {
        Value::Object(map) => {
            if !map.contains_key("discriminator")
                && let Some(members) = map.get("oneOf").and_then(Value::as_array)
                && let Some(disc) = discriminator_for(members, schemas)
            {
                map.insert("discriminator".to_owned(), disc);
            }
            for child in map.values_mut() {
                add_discriminators(child, schemas);
            }
        }
        Value::Array(items) => {
            for child in items {
                add_discriminators(child, schemas);
            }
        }
        _ => {}
    }
}

fn resolve<'a>(schema: &'a Value, schemas: &'a Value) -> Option<&'a Value> {
    match schema.get("$ref").and_then(Value::as_str) {
        Some(r) => schemas.get(r.strip_prefix("#/components/schemas/")?),
        None => Some(schema),
    }
}

fn tag_value(schema: &Value, property: &str) -> Option<String> {
    let required = schema.get("required")?.as_array()?;
    if !required.iter().any(|r| r == property) {
        return None;
    }
    let prop = schema.get("properties")?.get(property)?;
    if let Some(Value::String(c)) = prop.get("const") {
        return Some(c.clone());
    }
    match prop.get("enum")?.as_array()?.as_slice() {
        [Value::String(v)] => Some(v.clone()),
        _ => None,
    }
}

fn discriminator_for(members: &[Value], schemas: &Value) -> Option<Value> {
    let first = resolve(members.first()?, schemas)?;
    let candidates: Vec<&String> = first.get("properties")?.as_object()?.keys().collect();
    'candidates: for property in candidates {
        let mut mapping = Map::new();
        let mut all_refs = true;
        for member in members {
            let resolved = resolve(member, schemas)?;
            let Some(tag) = tag_value(resolved, property) else {
                continue 'candidates;
            };
            match member.get("$ref") {
                Some(r) => {
                    mapping.insert(tag, r.clone());
                }
                None => all_refs = false,
            }
        }
        let mut disc = json!({ "propertyName": property });
        if all_refs {
            disc["mapping"] = Value::Object(mapping);
        }
        return Some(disc);
    }
    None
}

/// Recursively sorts object keys, making output independent of map implementations.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), canonical(&map[key]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// Checks a built document against the conventions. Returns one message per violation,
/// sorted.
pub fn lint(doc: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut ids = std::collections::BTreeSet::new();
    let empty = Map::new();
    let paths = doc["paths"].as_object().unwrap_or(&empty);
    for (path, item) in paths {
        if !path.starts_with(API_PREFIX) {
            out.push(format!("{path}: not under {API_PREFIX}"));
        }
        let Some(item) = item.as_object() else {
            continue;
        };
        for (method, op) in item {
            if !HTTP_METHODS.contains(&method.as_str()) {
                continue;
            }
            let at = format!("{} {path}", method.to_uppercase());
            match op["operationId"].as_str() {
                Some(id) if !id.is_empty() => {
                    if !ids.insert(id.to_owned()) {
                        out.push(format!("duplicate operationId `{id}`"));
                    }
                }
                _ => out.push(format!("{at}: missing operationId")),
            }
            if op["tags"].as_array().is_none_or(Vec::is_empty) {
                out.push(format!("{at}: missing tags"));
            }
            lint_operation(doc, op, &at, &mut out);
        }
    }
    lint_refs(doc, doc, &mut out);
    out.sort();
    out
}

fn lint_operation(doc: &Value, op: &Value, at: &str, out: &mut Vec<String>) {
    if let Some(stream) = op.get(STREAM_EXTENSION) {
        for key in ["payload", "frame"] {
            let name = stream[key].as_str().unwrap_or_default();
            if doc["components"]["schemas"].get(name).is_none() {
                out.push(format!(
                    "{at}: stream {key} schema `{name}` is not a component"
                ));
            }
        }
        return;
    }
    if let Some(content) = op
        .pointer("/requestBody/content")
        .and_then(Value::as_object)
    {
        for media in content.keys() {
            if media != MSGPACK && media != ZIP {
                out.push(format!("{at}: request media type `{media}`"));
            }
        }
        for status in ["413", "415", "422"] {
            if op["responses"].get(status).is_none() {
                out.push(format!("{at}: body without a {status} response"));
            }
        }
    }
    let empty = Map::new();
    let responses = op["responses"].as_object().unwrap_or(&empty);
    if !responses.keys().any(|s| s.starts_with('2')) {
        out.push(format!("{at}: no success response"));
    }
    for (status, response) in responses {
        let response = match response["$ref"].as_str() {
            Some(r) => doc.pointer(&r.replacen('#', "", 1)).unwrap_or(&Value::Null),
            None => response,
        };
        let Some(content) = response["content"].as_object() else {
            continue;
        };
        for media in content.keys() {
            let ok = if status.starts_with('2') {
                media == MSGPACK || media == ZIP
            } else {
                media == PROBLEM_MSGPACK
            };
            if !ok {
                out.push(format!("{at}: {status} response media type `{media}`"));
            }
        }
    }
}

fn lint_refs(doc: &Value, value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref")
                && doc.pointer(&r.replacen('#', "", 1)).is_none()
            {
                out.push(format!("unresolved $ref `{r}`"));
            }
            for child in map.values() {
                lint_refs(doc, child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                lint_refs(doc, child, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
