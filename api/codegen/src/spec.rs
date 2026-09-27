//! The subset of OpenAPI the generator reads, parsed from `serde_json::Value`.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::Error;

const MSGPACK: &str = "application/msgpack";
const STREAM_EXTENSION: &str = "x-strata-stream";
const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "patch", "head", "options", "trace",
];

/// Where a parameter goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Location {
    Path,
    Query,
    Header,
}

#[derive(Debug, Clone)]
pub(crate) struct Param {
    pub name: String,
    pub location: Location,
    pub required: bool,
    pub schema: Value,
}

#[derive(Debug, Clone)]
pub(crate) struct Operation {
    pub id: String,
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub params: Vec<Param>,
    /// Request body schema (MessagePack) and whether it is required.
    pub body: Option<(Value, bool)>,
    /// Success response schema; `None` for empty success responses.
    pub response: Option<Value>,
    pub auth: bool,
    /// Payload component of a stream operation.
    pub stream: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct Api {
    /// Component schemas, sorted by name.
    pub schemas: BTreeMap<String, Value>,
    /// Operations sorted by path, then method.
    pub operations: Vec<Operation>,
}

fn spec_err(msg: impl Into<String>) -> Error {
    Error::Spec(msg.into())
}

impl Api {
    pub fn parse(doc: &Value) -> Result<Self, Error> {
        check_refs(doc, doc)?;
        let schemas: BTreeMap<String, Value> = doc
            .pointer("/components/schemas")
            .and_then(Value::as_object)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        let root_security = doc.get("security");
        let paths: BTreeMap<&String, &Value> = doc
            .get("paths")
            .and_then(Value::as_object)
            .ok_or_else(|| spec_err("missing `paths`"))?
            .iter()
            .collect();
        let mut operations = Vec::new();
        for (path, item) in paths {
            for method in METHODS {
                if let Some(op) = item.get(method) {
                    operations.push(parse_operation(doc, path, method, item, op, root_security)?);
                }
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for op in &operations {
            if !ids.insert(op.id.clone()) {
                return Err(spec_err(format!("duplicate operationId `{}`", op.id)));
            }
        }
        Ok(Self {
            schemas,
            operations,
        })
    }
}

fn resolve<'a>(doc: &'a Value, value: &'a Value) -> &'a Value {
    match value.get("$ref").and_then(Value::as_str) {
        Some(r) => doc.pointer(&r.replacen('#', "", 1)).unwrap_or(value),
        None => value,
    }
}

fn parse_operation(
    doc: &Value,
    path: &str,
    method: &str,
    item: &Value,
    op: &Value,
    root_security: Option<&Value>,
) -> Result<Operation, Error> {
    let at = format!("{} {path}", method.to_uppercase());
    let id = op
        .get("operationId")
        .and_then(Value::as_str)
        .ok_or_else(|| spec_err(format!("{at}: missing operationId")))?
        .to_owned();
    let text = |key: &str| op.get(key).and_then(Value::as_str).map(str::to_owned);

    let mut params = Vec::new();
    let item_params = item.get("parameters").and_then(Value::as_array);
    let op_params = op.get("parameters").and_then(Value::as_array);
    for p in item_params.into_iter().chain(op_params).flatten() {
        let p = resolve(doc, p);
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| spec_err(format!("{at}: parameter without name")))?;
        let location = match p.get("in").and_then(Value::as_str) {
            Some("path") => Location::Path,
            Some("query") => Location::Query,
            Some("header") => Location::Header,
            other => {
                return Err(Error::Unsupported(format!(
                    "{at}: parameter `{name}` in {other:?}"
                )));
            }
        };
        params.push(Param {
            name: name.to_owned(),
            location,
            required: location == Location::Path
                || p.get("required").and_then(Value::as_bool).unwrap_or(false),
            schema: p.get("schema").cloned().unwrap_or(Value::Bool(true)),
        });
    }
    // Path parameters in template order, then the rest in declaration order.
    let order = |p: &Param| match p.location {
        Location::Path => path.find(&format!("{{{}}}", p.name)).unwrap_or(usize::MAX),
        _ => usize::MAX,
    };
    params.sort_by_key(|p| (p.location != Location::Path, order(p)));

    let security = op.get("security").or(root_security);
    let auth = security
        .and_then(Value::as_array)
        .is_some_and(|reqs| {
            !reqs.is_empty()
                && !reqs
                    .iter()
                    .any(|r| r.as_object().is_some_and(serde_json::Map::is_empty))
        });

    if let Some(stream) = op.get(STREAM_EXTENSION) {
        let payload = stream
            .get("payload")
            .and_then(Value::as_str)
            .ok_or_else(|| spec_err(format!("{at}: {STREAM_EXTENSION} without payload")))?;
        return Ok(Operation {
            id,
            method: method.to_owned(),
            path: path.to_owned(),
            summary: text("summary"),
            description: text("description"),
            params: params
                .into_iter()
                .filter(|p| p.location == Location::Path)
                .collect(),
            body: None,
            response: None,
            auth,
            stream: Some(payload.to_owned()),
        });
    }

    let body = match op.get("requestBody").map(|b| resolve(doc, b)) {
        None => None,
        Some(b) => {
            let content = b
                .get("content")
                .and_then(Value::as_object)
                .ok_or_else(|| spec_err(format!("{at}: request body without content")))?;
            let media = content.get(MSGPACK).ok_or_else(|| {
                Error::Unsupported(format!(
                    "{at}: request media types {:?}",
                    content.keys().collect::<Vec<_>>()
                ))
            })?;
            let schema = media.get("schema").cloned().unwrap_or(Value::Bool(true));
            let required = b.get("required").and_then(Value::as_bool).unwrap_or(false);
            Some((schema, required))
        }
    };

    let mut success: Vec<Option<Value>> = Vec::new();
    if let Some(responses) = op.get("responses").and_then(Value::as_object) {
        for (status, response) in responses {
            if !status.starts_with('2') {
                continue;
            }
            let response = resolve(doc, response);
            match response.get("content").and_then(Value::as_object) {
                None => success.push(None),
                Some(content) => {
                    let media = content.get(MSGPACK).ok_or_else(|| {
                        Error::Unsupported(format!(
                            "{at} {status}: response media types {:?}",
                            content.keys().collect::<Vec<_>>()
                        ))
                    })?;
                    success.push(Some(media.get("schema").cloned().unwrap_or(Value::Bool(true))));
                }
            }
        }
    }
    success.dedup();
    let response = match success.as_slice() {
        [] => return Err(spec_err(format!("{at}: no success response"))),
        [single] => single.clone(),
        _ => {
            return Err(Error::Unsupported(format!(
                "{at}: success responses with different bodies"
            )));
        }
    };

    Ok(Operation {
        id,
        method: method.to_owned(),
        path: path.to_owned(),
        summary: text("summary"),
        description: text("description"),
        params,
        body,
        response,
        auth,
        stream: None,
    })
}

/// Every `$ref` must resolve inside the document (typify would panic otherwise).
fn check_refs(doc: &Value, value: &Value) -> Result<(), Error> {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                let resolves = r.starts_with("#/") && doc.pointer(&r.replacen('#', "", 1)).is_some();
                if !resolves {
                    return Err(spec_err(format!("unresolved $ref `{r}`")));
                }
            }
            map.values().try_for_each(|v| check_refs(doc, v))
        }
        Value::Array(items) => items.iter().try_for_each(|v| check_refs(doc, v)),
        _ => Ok(()),
    }
}
