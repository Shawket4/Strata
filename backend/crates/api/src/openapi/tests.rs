use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

fn committed_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../api/openapi.json")
}

#[test]
fn production_document_passes_lint() {
    assert_eq!(lint(&document()), Vec::<String>::new());
}

#[cfg(feature = "test-support")]
#[test]
fn demo_document_passes_lint() {
    assert_eq!(
        lint(&crate::testing::demo::document()),
        Vec::<String>::new()
    );
}

#[test]
fn committed_openapi_json_is_up_to_date() {
    let committed = std::fs::read_to_string(committed_path()).unwrap_or_default();
    assert!(
        committed == to_pretty_json(&document()),
        "api/openapi.json is stale: run api/generate.sh"
    );
}

#[test]
fn output_is_deterministic_and_canonical() {
    let a = to_pretty_json(&document());
    let b = to_pretty_json(&document());
    assert_eq!(a, b);
    assert!(a.ends_with("}\n"));
    // Re-canonicalising a parsed copy changes nothing: keys are already sorted.
    let reparsed: Value = serde_json::from_str(&a).expect("valid JSON");
    assert_eq!(to_pretty_json(&reparsed), a);
}

#[test]
fn write_creates_the_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("openapi.json");
    write(&path).expect("writes");
    assert_eq!(
        std::fs::read_to_string(&path).expect("reads"),
        to_pretty_json(&document())
    );
}

#[test]
fn health_operation_is_public_msgpack_and_carries_standard_responses() {
    let doc = document();
    let op = &doc["paths"]["/api/v1/health"]["get"];
    assert_eq!(
        op,
        &json!({
            "operationId": "health",
            "summary": "Liveness probe.",
            "tags": ["system"],
            "security": [{}],
            "responses": {
                "200": {
                    "description": "The server is alive.",
                    "content": { "application/vnd.msgpack": { "schema": { "$ref": "#/components/schemas/Health" } } },
                },
                "406": { "$ref": "#/components/responses/NotAcceptable" },
                "500": { "$ref": "#/components/responses/Internal" },
            },
        })
    );
    assert_eq!(doc["security"], json!([{ "bearer": [] }]));
    assert_eq!(
        doc["components"]["securitySchemes"]["bearer"]["scheme"],
        json!("bearer")
    );
    assert_eq!(doc["openapi"], json!("3.1.0"));
}

#[test]
fn body_operations_get_body_problem_responses_and_secured_ones_401() {
    let mut op = json!({
        "requestBody": { "content": { "application/json": { "schema": { "type": "string" } } } },
        "parameters": [{ "in": "path", "name": "id" }],
        "responses": {
            "201": { "description": "ok", "content": { "application/json": {} } },
            "409": { "description": "conflict", "content": { "application/json": {} } },
        },
    });
    apply_operation_conventions(&mut op);
    let keys: Vec<&String> = op["responses"].as_object().expect("map").keys().collect();
    let mut keys: Vec<&str> = keys.into_iter().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "201", "401", "403", "404", "406", "409", "413", "415", "422", "500"
        ]
    );
    assert_eq!(op["responses"]["422"], response_ref("InvalidBody"));
    assert_eq!(op["responses"]["403"], response_ref("Restricted"));
    assert!(op["requestBody"]["content"].get(MSGPACK).is_some());
    assert!(op["responses"]["201"]["content"].get(MSGPACK).is_some());
    assert!(
        op["responses"]["409"]["content"]
            .get(PROBLEM_MSGPACK)
            .is_some()
    );
}

#[test]
fn lint_reports_each_violation() {
    let doc = json!({
        "paths": {
            "/other": { "get": { "operationId": "a", "tags": ["t"], "responses": { "200": {} } } },
            "/api/v1/x": {
                "get": { "operationId": "a", "responses": {
                    "200": { "content": { "application/json": {} } },
                    "404": { "content": { "application/vnd.msgpack": {} } },
                } },
                "post": { "tags": ["t"],
                    "requestBody": { "content": { "text/plain": {} } },
                    "responses": { "400": { "$ref": "#/components/responses/Missing" } } },
            },
        },
    });
    assert_eq!(
        lint(&doc),
        vec![
            "/other: not under /api/v1".to_owned(),
            "GET /api/v1/x: 200 response media type `application/json`".to_owned(),
            "GET /api/v1/x: 404 response media type `application/vnd.msgpack`".to_owned(),
            "GET /api/v1/x: missing tags".to_owned(),
            "POST /api/v1/x: body without a 413 response".to_owned(),
            "POST /api/v1/x: body without a 415 response".to_owned(),
            "POST /api/v1/x: body without a 422 response".to_owned(),
            "POST /api/v1/x: missing operationId".to_owned(),
            "POST /api/v1/x: no success response".to_owned(),
            "POST /api/v1/x: request media type `text/plain`".to_owned(),
            "duplicate operationId `a`".to_owned(),
            "unresolved $ref `#/components/responses/Missing`".to_owned(),
        ]
    );
}

#[test]
fn tagged_one_of_gets_a_discriminator_with_mapping_for_refs() {
    let schemas = json!({
        "A": { "type": "object", "required": ["kind"], "properties": { "kind": { "enum": ["a"] } } },
        "B": { "type": "object", "required": ["kind"], "properties": { "kind": { "const": "b" } } },
    });
    let mut doc = json!({
        "refs": { "oneOf": [{ "$ref": "#/components/schemas/A" }, { "$ref": "#/components/schemas/B" }] },
        "inline": { "oneOf": [
            { "type": "object", "required": ["t", "x"], "properties": { "x": {}, "t": { "enum": ["one"] } } },
            { "type": "object", "required": ["t"], "properties": { "t": { "enum": ["two"] } } },
        ] },
        "option": { "oneOf": [{ "type": "null" }, { "$ref": "#/components/schemas/A" }] },
    });
    add_discriminators(&mut doc, &schemas);
    assert_eq!(
        doc["refs"]["discriminator"],
        json!({ "propertyName": "kind", "mapping": {
            "a": "#/components/schemas/A", "b": "#/components/schemas/B" } })
    );
    assert_eq!(
        doc["inline"]["discriminator"],
        json!({ "propertyName": "t" })
    );
    assert_eq!(doc["option"].get("discriminator"), None);
}

#[cfg(feature = "test-support")]
#[test]
fn demo_tagged_enum_and_stream_frames_are_documented() {
    let doc = crate::testing::demo::document();
    let schemas = &doc["components"]["schemas"];
    assert_eq!(
        schemas["Shape"]["discriminator"],
        json!({ "propertyName": "type" })
    );
    assert_eq!(
        schemas["DemoTickFrame"]["discriminator"]["mapping"]["data"],
        json!("#/components/schemas/DemoTickDataFrame")
    );
    assert_eq!(
        doc["paths"]["/api/v1/demo/ticks"]["get"][STREAM_EXTENSION],
        json!({ "payload": "DemoTick", "frame": "DemoTickFrame" })
    );
    assert_eq!(
        schemas["Binary"],
        json!({
        "type": "string", "format": "binary",
        "description": "Raw bytes, carried as MessagePack `bin`." })
    );
}
