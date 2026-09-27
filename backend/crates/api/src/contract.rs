//! Contract conformance (PLAN §7.6, §16.3): decode MessagePack bodies and validate them
//! against the OpenAPI document with JSON Schema 2020-12.
//!
//! Reused by every API test: send a request, then
//! `Contract::production().validate_response("operation_id", status, content_type, &body)`.
//!
//! MessagePack is converted to JSON for validation: `bin` becomes a base64 string (the
//! contract types bytes as `type: string, format: binary`), map keys must be strings, and
//! extension types, NaN and infinities are violations. Validation is strict: an object schema
//! with `properties` rejects properties it does not declare, so undocumented fields are caught.

use base64::Engine;
use serde_json::{Map, Number, Value, json};

use crate::wire::ws::STREAM_EXTENSION;
use crate::wire::{DecodeLimits, scan};

/// Why a message does not conform to the contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContractViolation {
    /// No operation has this id.
    #[error("unknown operation `{0}`")]
    UnknownOperation(String),
    /// No component schema has this name.
    #[error("unknown component schema `{0}`")]
    UnknownSchema(String),
    /// The status is not documented for the operation.
    #[error("`{operation}` does not document status {status}")]
    UndocumentedStatus {
        /// Operation id.
        operation: String,
        /// Actual status.
        status: u16,
    },
    /// The content type does not match the documented one.
    #[error("`{operation}` {status}: content type {actual:?}, documented {expected:?}")]
    ContentType {
        /// Operation id.
        operation: String,
        /// Status.
        status: u16,
        /// Documented media types (empty = no body).
        expected: Vec<String>,
        /// Actual `Content-Type` (essence), if any.
        actual: Option<String>,
    },
    /// The body is not a single well-formed MessagePack value representable as JSON.
    #[error("body is not valid MessagePack for the contract: {0}")]
    NotMsgPack(String),
    /// The decoded body violates its schema.
    #[error("schema violations at {location}: {errors:?}")]
    Schema {
        /// Operation/status or schema name.
        location: String,
        /// One entry per violation: `<instance path>: <message>`.
        errors: Vec<String>,
    },
}

/// A loaded contract.
#[derive(Debug, Clone)]
pub struct Contract {
    doc: Value,
    strict_components: Value,
}

impl Contract {
    /// The production contract ([`crate::openapi::document`]).
    pub fn production() -> Self {
        Self::new(crate::openapi::document())
    }

    /// A contract from any built document (e.g. the demo document).
    pub fn new(doc: Value) -> Self {
        let mut components = doc.get("components").cloned().unwrap_or_else(|| json!({}));
        let in_all_of = all_of_refs(&doc);
        if let Some(schemas) = components.get_mut("schemas").and_then(Value::as_object_mut) {
            for (name, schema) in schemas.iter_mut() {
                if !in_all_of.contains(name) {
                    make_strict(schema);
                }
            }
        }
        Self {
            doc,
            strict_components: components,
        }
    }

    /// The document.
    pub fn document(&self) -> &Value {
        &self.doc
    }

    /// `(method, path, operation)` of an operation id.
    pub fn operation(&self, operation_id: &str) -> Option<(&str, &str, &Value)> {
        let paths = self.doc.get("paths")?.as_object()?;
        for (path, item) in paths {
            for (method, op) in item.as_object()? {
                if op.get("operationId").and_then(Value::as_str) == Some(operation_id) {
                    return Some((method, path, op));
                }
            }
        }
        None
    }

    /// Validates a response of `operation_id`; returns the body as JSON (`Null` when empty).
    pub fn validate_response(
        &self,
        operation_id: &str,
        status: u16,
        content_type: Option<&str>,
        body: &[u8],
    ) -> Result<Value, ContractViolation> {
        let (_, _, op) = self
            .operation(operation_id)
            .ok_or_else(|| ContractViolation::UnknownOperation(operation_id.to_owned()))?;
        let responses = &op["responses"];
        let response = responses
            .get(status.to_string())
            .or_else(|| responses.get("default"))
            .ok_or_else(|| ContractViolation::UndocumentedStatus {
                operation: operation_id.to_owned(),
                status,
            })?;
        let response = self.deref(response);
        let content = response.get("content").and_then(Value::as_object);
        let actual = content_type.map(|c| {
            c.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        });
        let expected: Vec<String> = content
            .map(|c| c.keys().cloned().collect())
            .unwrap_or_default();
        let mismatch = || ContractViolation::ContentType {
            operation: operation_id.to_owned(),
            status,
            expected: expected.clone(),
            actual: actual.clone(),
        };
        let Some(content) = content else {
            return if body.is_empty() {
                Ok(Value::Null)
            } else {
                Err(mismatch())
            };
        };
        let media = actual
            .as_deref()
            .and_then(|a| content.get(a))
            .ok_or_else(mismatch)?;
        let json = msgpack_to_json(body)?;
        if let Some(schema) = media.get("schema") {
            self.validate(schema, &json, &format!("{operation_id} {status}"))?;
        }
        Ok(json)
    }

    /// Validates a request body of `operation_id` (MessagePack media type).
    pub fn validate_request(
        &self,
        operation_id: &str,
        body: &[u8],
    ) -> Result<Value, ContractViolation> {
        let (_, _, op) = self
            .operation(operation_id)
            .ok_or_else(|| ContractViolation::UnknownOperation(operation_id.to_owned()))?;
        let schema = op
            .pointer(&format!(
                "/requestBody/content/{}",
                crate::wire::MSGPACK.replace('/', "~1")
            ))
            .and_then(|m| m.get("schema"))
            .ok_or_else(|| {
                ContractViolation::UnknownOperation(format!("{operation_id} (no body)"))
            })?;
        let json = msgpack_to_json(body)?;
        self.validate(schema, &json, &format!("{operation_id} request"))?;
        Ok(json)
    }

    /// Validates a body against a named component schema.
    pub fn validate_component(&self, name: &str, body: &[u8]) -> Result<Value, ContractViolation> {
        if self
            .doc
            .pointer(&format!("/components/schemas/{name}"))
            .is_none()
        {
            return Err(ContractViolation::UnknownSchema(name.to_owned()));
        }
        let json = msgpack_to_json(body)?;
        self.validate(
            &json!({ "$ref": format!("#/components/schemas/{name}") }),
            &json,
            name,
        )?;
        Ok(json)
    }

    /// Validates a WebSocket frame of the stream operation `operation_id`.
    pub fn validate_frame(
        &self,
        operation_id: &str,
        frame: &[u8],
    ) -> Result<Value, ContractViolation> {
        let (_, _, op) = self
            .operation(operation_id)
            .ok_or_else(|| ContractViolation::UnknownOperation(operation_id.to_owned()))?;
        let name = op
            .get(STREAM_EXTENSION)
            .and_then(|s| s.get("frame"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                ContractViolation::UnknownOperation(format!("{operation_id} (not a stream)"))
            })?;
        self.validate_component(name, frame)
    }

    fn deref<'a>(&'a self, value: &'a Value) -> &'a Value {
        match value.get("$ref").and_then(Value::as_str) {
            Some(r) => self
                .doc
                .pointer(&r.replacen('#', "", 1))
                .unwrap_or(&Value::Null),
            None => value,
        }
    }

    fn validate(
        &self,
        schema: &Value,
        instance: &Value,
        location: &str,
    ) -> Result<(), ContractViolation> {
        let mut root = schema.clone();
        if let Some(obj) = root.as_object_mut() {
            obj.insert("components".to_owned(), self.strict_components.clone());
        }
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .should_validate_formats(true)
            .should_ignore_unknown_formats(true)
            .with_format("ulid", |s: &str| s.parse::<ulid::Ulid>().is_ok())
            .build(&root)
            .map_err(|e| ContractViolation::Schema {
                location: location.to_owned(),
                errors: vec![format!("invalid schema: {e}")],
            })?;
        let errors: Vec<String> = validator
            .iter_errors(instance)
            .map(|e| format!("{}: {e}", e.instance_path()))
            .collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ContractViolation::Schema {
                location: location.to_owned(),
                errors,
            })
        }
    }
}

/// Names of component schemas referenced directly from an `allOf` (left non-strict, since
/// `unevaluatedProperties` would reject the sibling members' properties).
fn all_of_refs(doc: &Value) -> std::collections::BTreeSet<String> {
    fn walk(v: &Value, out: &mut std::collections::BTreeSet<String>) {
        match v {
            Value::Object(map) => {
                if let Some(Value::Array(members)) = map.get("allOf") {
                    for m in members {
                        if let Some(name) = m
                            .get("$ref")
                            .and_then(Value::as_str)
                            .and_then(|r| r.strip_prefix("#/components/schemas/"))
                        {
                            out.insert(name.to_owned());
                        }
                    }
                }
                map.values().for_each(|c| walk(c, out));
            }
            Value::Array(items) => items.iter().for_each(|c| walk(c, out)),
            _ => {}
        }
    }
    let mut out = std::collections::BTreeSet::new();
    walk(doc, &mut out);
    out
}

/// Adds `unevaluatedProperties: false` to object schemas that list `properties`, except
/// direct `allOf` members.
fn make_strict(schema: &mut Value) {
    let Some(map) = schema.as_object_mut() else {
        return;
    };
    if map.contains_key("properties")
        && !map.contains_key("additionalProperties")
        && !map.contains_key("unevaluatedProperties")
    {
        map.insert("unevaluatedProperties".to_owned(), Value::Bool(false));
    }
    for (key, child) in map.iter_mut() {
        match key.as_str() {
            "allOf" => {}
            "properties" => {
                if let Some(props) = child.as_object_mut() {
                    props.values_mut().for_each(make_strict);
                }
            }
            _ => match child {
                Value::Array(items) => items.iter_mut().for_each(make_strict),
                Value::Object(_) => make_strict(child),
                _ => {}
            },
        }
    }
}

/// Converts one MessagePack document to JSON for schema validation (see module docs).
pub fn msgpack_to_json(bytes: &[u8]) -> Result<Value, ContractViolation> {
    let limits = DecodeLimits {
        max_depth: 128,
        max_str_len: u32::MAX,
        max_bin_len: u32::MAX,
        max_array_len: u32::MAX,
        max_map_len: u32::MAX,
    };
    scan(bytes, &limits).map_err(|v| ContractViolation::NotMsgPack(v.to_string()))?;
    let mut rd = bytes;
    let value = rmpv::decode::read_value(&mut rd)
        .map_err(|e| ContractViolation::NotMsgPack(e.to_string()))?;
    to_json(&value)
}

fn to_json(value: &rmpv::Value) -> Result<Value, ContractViolation> {
    use rmpv::Value as M;
    Ok(match value {
        M::Nil => Value::Null,
        M::Boolean(b) => Value::Bool(*b),
        M::Integer(i) => match (i.as_u64(), i.as_i64()) {
            (Some(u), _) => Value::Number(u.into()),
            (None, Some(s)) => Value::Number(s.into()),
            (None, None) => {
                return Err(ContractViolation::NotMsgPack("integer out of range".into()));
            }
        },
        M::F32(f) => float(f64::from(*f))?,
        M::F64(f) => float(*f)?,
        M::String(s) => Value::String(
            s.as_str()
                .ok_or_else(|| ContractViolation::NotMsgPack("invalid UTF-8".into()))?
                .to_owned(),
        ),
        M::Binary(b) => Value::String(base64::engine::general_purpose::STANDARD.encode(b)),
        M::Array(items) => Value::Array(items.iter().map(to_json).collect::<Result<_, _>>()?),
        M::Map(entries) => {
            let mut map = Map::new();
            for (k, v) in entries {
                let key = k
                    .as_str()
                    .ok_or_else(|| ContractViolation::NotMsgPack("non-string map key".into()))?;
                if map.insert(key.to_owned(), to_json(v)?).is_some() {
                    return Err(ContractViolation::NotMsgPack(format!(
                        "duplicate map key `{key}`"
                    )));
                }
            }
            Value::Object(map)
        }
        M::Ext(..) => return Err(ContractViolation::NotMsgPack("extension type".into())),
    })
}

fn float(f: f64) -> Result<Value, ContractViolation> {
    Number::from_f64(f)
        .map(Value::Number)
        .ok_or_else(|| ContractViolation::NotMsgPack("non-finite float".into()))
}
