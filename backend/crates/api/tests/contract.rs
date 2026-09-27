//! The conformance helper itself: conversions, strictness and every violation kind.

use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use strata_api::contract::{Contract, ContractViolation, msgpack_to_json};
use strata_api::testing::demo;
use strata_api::wire::ws::Frame;
use strata_api::wire::{MSGPACK, PROBLEM_MSGPACK, Problem, ProblemType, encode};

fn contract() -> Contract {
    Contract::new(demo::document())
}

fn mp(value: &Value) -> Vec<u8> {
    encode(value).expect("encodes")
}

fn widget_json() -> Value {
    json!({
        "id": "01J8ZK3M4X7Q9V2B6C8D0E1F2G",
        "name": "w",
        "shape": { "type": "circle", "radius": 1.5 },
        "tags": [],
        "blob": "",
        "version": "v1.1",
        "created": "2026-09-21T14:13:21Z",
    })
}

#[test]
fn msgpack_converts_to_json_with_bin_as_base64() {
    // {"b": bin[de ad], "n": -1, "f": 0.5, "a": [nil, true]}
    let bytes = [
        0x84, 0xa1, b'b', 0xc4, 0x02, 0xde, 0xad, 0xa1, b'n', 0xff, 0xa1, b'f', 0xcb, 0x3f, 0xe0,
        0, 0, 0, 0, 0, 0, 0xa1, b'a', 0x92, 0xc0, 0xc3,
    ];
    assert_eq!(
        msgpack_to_json(&bytes),
        Ok(json!({ "b": "3q0=", "n": -1, "f": 0.5, "a": [null, true] }))
    );
}

#[test]
fn msgpack_that_json_cannot_represent_is_rejected() {
    let nan = [0xcb, 0x7f, 0xf8, 0, 0, 0, 0, 0, 0];
    let cases: [(&[u8], &str); 5] = [
        (&[0x81, 0x01, 0x02], "non-string map key"),
        (&[0x82, 0xa1, b'k', 0x01, 0xa1, b'k', 0x02], "duplicate map key `k`"),
        (&nan, "non-finite float"),
        (&[0xd4, 0x01, 0x00], "MessagePack extension types are not allowed"),
        (&[0x01, 0x01], "1 trailing bytes after the MessagePack value"),
    ];
    for (bytes, message) in cases {
        assert_eq!(
            msgpack_to_json(bytes),
            Err(ContractViolation::NotMsgPack(message.to_owned()))
        );
    }
}

#[test]
fn a_documented_response_validates_and_is_returned_as_json() {
    let body = mp(&widget_json());
    assert_eq!(
        contract().validate_response("get_widget", 200, Some(MSGPACK), &body),
        Ok(widget_json())
    );
}

#[test]
fn undocumented_fields_wrong_types_and_bad_formats_are_violations() {
    let mut extra = widget_json();
    extra["secret"] = json!(1);
    let mut wrong = widget_json();
    wrong["tags"] = json!("x");
    let mut bad_time = widget_json();
    bad_time["created"] = json!("yesterday");
    let mut bad_id = widget_json();
    bad_id["id"] = json!("not-a-ulid");
    let mut bad_variant = widget_json();
    bad_variant["shape"] = json!({ "type": "hexagon" });
    for (value, path) in [
        (extra, ""),
        (wrong, "/tags"),
        (bad_time, "/created"),
        (bad_id, "/id"),
        (bad_variant, "/shape"),
    ] {
        let err = contract()
            .validate_response("get_widget", 200, Some(MSGPACK), &mp(&value))
            .expect_err("must violate");
        let ContractViolation::Schema { location, errors } = err else {
            panic!("expected a schema violation, got {err:?}");
        };
        assert_eq!(location, "get_widget 200");
        assert!(
            errors.iter().any(|e| e.starts_with(&format!("{path}: "))),
            "{path} not in {errors:?}"
        );
    }
}

#[test]
fn content_type_status_and_operation_are_checked() {
    let c = contract();
    let body = mp(&widget_json());
    assert_eq!(
        c.validate_response("get_widget", 200, Some("application/json"), &body),
        Err(ContractViolation::ContentType {
            operation: "get_widget".to_owned(),
            status: 200,
            expected: vec![MSGPACK.to_owned()],
            actual: Some("application/json".to_owned()),
        })
    );
    assert_eq!(
        c.validate_response("get_widget", 418, Some(MSGPACK), &body),
        Err(ContractViolation::UndocumentedStatus {
            operation: "get_widget".to_owned(),
            status: 418,
        })
    );
    assert_eq!(
        c.validate_response("nope", 200, Some(MSGPACK), &body),
        Err(ContractViolation::UnknownOperation("nope".to_owned()))
    );
    assert_eq!(
        c.validate_response("delete_widget", 204, None, &[]),
        Ok(Value::Null)
    );
    assert!(matches!(
        c.validate_response("delete_widget", 204, Some(MSGPACK), &[0xc0]),
        Err(ContractViolation::ContentType { .. })
    ));
}

#[test]
fn problem_responses_resolve_shared_response_refs() {
    let problem = Problem::new(ProblemType::NotFound).to_msgpack();
    let json = contract()
        .validate_response(
            "get_widget",
            404,
            Some("application/problem+msgpack; charset=binary"),
            &problem,
        )
        .expect("conforms");
    assert_eq!(
        json,
        json!({ "type": "not_found", "title": "Not found", "status": 404 })
    );
    assert!(matches!(
        contract().validate_response("get_widget", 404, Some(MSGPACK), &problem),
        Err(ContractViolation::ContentType { .. })
    ));
    let _ = PROBLEM_MSGPACK;
}

#[test]
fn requests_and_components_validate_too() {
    let c = contract();
    let rename = mp(&json!({ "name": "x" }));
    assert_eq!(
        c.validate_request("rename_widget", &rename),
        Ok(json!({ "name": "x" }))
    );
    assert!(c.validate_request("rename_widget", &mp(&json!({}))).is_err());
    assert_eq!(
        c.validate_component("Nope", &rename),
        Err(ContractViolation::UnknownSchema("Nope".to_owned()))
    );
}

#[test]
fn every_frame_kind_validates_against_the_stream_frame_schema() {
    let c = contract();
    let frames: Vec<Frame<demo::DemoTick>> = vec![
        Frame::Data {
            seq: 1,
            payload: demo::DemoTick {
                n: 1,
                label: "a".to_owned(),
            },
        },
        Frame::Error {
            seq: 1,
            problem: Problem::new(ProblemType::Internal),
        },
        Frame::Reset { seq: 1 },
        Frame::End { seq: 1 },
    ];
    for frame in frames {
        let bytes = frame.encode().expect("encodes");
        c.validate_frame("demo_ticks", &bytes)
            .unwrap_or_else(|e| panic!("{frame:?}: {e}"));
    }
    let wrong = Frame::Data {
        seq: 1,
        payload: json!({ "n": "one" }),
    }
    .encode()
    .expect("encodes");
    assert!(matches!(
        c.validate_frame("demo_ticks", &wrong),
        Err(ContractViolation::Schema { .. })
    ));
}
