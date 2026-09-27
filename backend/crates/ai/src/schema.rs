//! Structured-output schemas: validation (jsonschema, draft 2020-12) and the reduced form sent
//! to the Messages API.
//!
//! Prompt schemas are written for local validation and may use keywords that the API's
//! structured outputs do not support (numeric bounds, string lengths, patterns, array sizes).
//! [`api_compatible`] strips those before sending; [`validate`] enforces the full schema on the
//! reply, and invalid replies are retried (see [`crate::AiService`]).

use serde_json::Value;

use crate::error::AiError;

/// One schema violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// JSON pointer of the offending value (`""` = root).
    pub instance_path: String,
    /// Content-free description (the kind of violation, never the value).
    pub kind: String,
    /// Full message including the offending value; only fed back to the model.
    pub detail: String,
}

impl Violation {
    /// `<instance path>: <kind>` — safe for logs and errors.
    pub fn summary(&self) -> String {
        let path = if self.instance_path.is_empty() {
            "/"
        } else {
            &self.instance_path
        };
        format!("{path}: {}", self.kind)
    }
}

/// Compiles `schema`.
pub fn compile(schema: &Value) -> Result<jsonschema::Validator, AiError> {
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(schema)
        .map_err(|e| AiError::InvalidSchema(e.to_string()))
}

/// Validates `instance`; `Err` lists every violation.
pub fn validate(
    validator: &jsonschema::Validator,
    instance: &Value,
) -> Result<(), Vec<Violation>> {
    let violations: Vec<Violation> = validator
        .iter_errors(instance)
        .map(|e| Violation {
            instance_path: e.instance_path().to_string(),
            kind: e.masked().to_string(),
            detail: e.to_string(),
        })
        .collect();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// Keywords the Messages API's structured outputs reject; enforced locally instead.
pub const API_UNSUPPORTED_KEYWORDS: &[&str] = &[
    "$schema",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "multipleOf",
    "minLength",
    "maxLength",
    "pattern",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minProperties",
    "maxProperties",
];

/// `schema` without [`API_UNSUPPORTED_KEYWORDS`], walking only schema positions (a property
/// *named* `pattern` is kept).
pub fn api_compatible(schema: &Value) -> Value {
    let mut out = schema.clone();
    strip(&mut out);
    out
}

fn strip(schema: &mut Value) {
    let Value::Object(map) = schema else {
        return;
    };
    for k in API_UNSUPPORTED_KEYWORDS {
        map.remove(*k);
    }
    for (key, child) in map.iter_mut() {
        match key.as_str() {
            "properties" | "$defs" | "definitions" | "patternProperties" => {
                if let Value::Object(members) = child {
                    members.values_mut().for_each(strip);
                }
            }
            "items" | "additionalProperties" | "not" | "contains" | "if" | "then" | "else" => {
                strip(child);
            }
            "anyOf" | "allOf" | "oneOf" | "prefixItems" => {
                if let Value::Array(members) = child {
                    members.iter_mut().for_each(strip);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn api_compatible_strips_constraints_in_schema_positions_only() {
        let schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "pattern": "^a$", "maxLength": 3},
                "n": {"type": "number", "minimum": 0, "maximum": 1},
                "xs": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}},
                "u": {"anyOf": [{"type": "string", "maxLength": 2}, {"type": "null"}]}
            },
            "required": ["pattern", "n", "xs", "u"],
            "additionalProperties": false
        });
        assert_eq!(
            api_compatible(&schema),
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "n": {"type": "number"},
                    "xs": {"type": "array", "items": {"type": "string"}},
                    "u": {"anyOf": [{"type": "string"}, {"type": "null"}]}
                },
                "required": ["pattern", "n", "xs", "u"],
                "additionalProperties": false
            })
        );
    }

    #[test]
    fn validate_reports_paths_and_masks_values_in_kinds() {
        let v = compile(&json!({
            "type": "object",
            "properties": {
                "confidence": {"type": "number", "minimum": 0, "maximum": 1},
                "reason": {"type": "string"}
            },
            "required": ["confidence", "reason"],
            "additionalProperties": false
        }))
        .expect("schema compiles");
        assert_eq!(validate(&v, &json!({"confidence": 0.5, "reason": 1})).map_err(|e| e.len()), Err(1));
        let errs = validate(&v, &json!({"confidence": 7.5})).expect_err("invalid");
        let summaries: Vec<String> = errs.iter().map(Violation::summary).collect();
        assert_eq!(
            summaries,
            vec![
                "/: \"reason\" is a required property".to_owned(),
                "/confidence: value is greater than the maximum of 1".to_owned(),
            ]
        );
        assert_eq!(
            errs[1].detail,
            "7.5 is greater than the maximum of 1".to_owned()
        );
        assert_eq!(validate(&v, &json!({"confidence": 1, "reason": "x"})), Ok(()));
    }

    #[test]
    fn invalid_schema_is_a_typed_error() {
        let err = compile(&json!({"type": 12})).expect_err("invalid schema");
        assert!(matches!(err, AiError::InvalidSchema(_)));
    }
}
