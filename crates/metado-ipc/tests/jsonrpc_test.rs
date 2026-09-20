//! Task 5.2: JSON-RPC 2.0 codec（request/response/notification；错误对象）。

use metado_ipc::jsonrpc::{ErrorObject, Request, Response, RpcRequest};

fn sample_params() -> serde_json::Value {
    serde_json::json!({ "perm": ["storage.read"] })
}

#[test]
fn test_request_encoding_roundtrip() {
    let req = Request::named("loadPlugin")
        .with_id(7u64)
        .with_params(sample_params());
    let json = req.to_json();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["method"], "loadPlugin");
    assert_eq!(json["id"], 7);
    let decoded = Request::from_json(&json.to_string()).unwrap();
    assert_eq!(decoded.id().and_then(|v| v.as_u64()), Some(7));
    assert_eq!(decoded.method(), "loadPlugin");
    assert_eq!(decoded.params(), &sample_params());
}

#[test]
fn test_notification_has_no_id_and_is_serialized_without_id() {
    let notif = Request::notification("dispatch", serde_json::json!([1, 2]));
    let json = notif.to_json();
    assert!(json.get("id").is_none());
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["method"], "dispatch");
}

#[test]
fn test_response_result_roundtrip() {
    let resp = Response::ok(3u64, serde_json::json!({ "ok": true }));
    let json = resp.to_json();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], 3);
    assert_eq!(json["result"]["ok"], true);
    let decoded = Response::from_json(&json.to_string()).unwrap();
    assert_eq!(decoded.id, serde_json::json!(3));
    assert!(decoded.result.is_some());
    assert!(decoded.error.is_none());
}

#[test]
fn test_response_error_roundtrip() {
    let err = ErrorObject::new(-32601, "method not found", Some(serde_json::json!({"m":"x"})));
    let resp = Response::error(1u64, err.clone());
    let decoded = Response::from_json(&resp.to_json().to_string()).unwrap();
    assert_eq!(decoded.id, serde_json::json!(1));
    assert!(decoded.result.is_none());
    let e = decoded.error.unwrap();
    assert_eq!(e.code, -32601);
    assert_eq!(e.message, "method not found");
}

#[test]
fn test_parse_invalid_json_errors() {
    assert!(Request::from_json("not json").is_err());
    assert!(Request::from_json("{\"jsonrpc\":\"2.0\"}").is_err());
    assert!(Response::from_json("{\"jsonrpc\":\"2.0\",\"id\":1}").is_err());
}

#[test]
fn test_standard_error_codes() {
    assert_eq!(ErrorObject::parse_error().code, -32700);
    assert_eq!(ErrorObject::invalid_request().code, -32600);
    assert_eq!(ErrorObject::method_not_found().code, -32601);
    assert_eq!(ErrorObject::invalid_params().code, -32602);
    assert_eq!(ErrorObject::internal_error().code, -32603);
}

#[test]
fn test_call_sugar_builds_request_with_string_id() {
    let call = RpcRequest::call("invoke", "req-1", serde_json::json!({ "entry": "entry:boot" }));
    assert_eq!(call.method, "invoke");
    assert_eq!(call.id, serde_json::json!("req-1"));
    assert_eq!(call.params["entry"], "entry:boot");
}