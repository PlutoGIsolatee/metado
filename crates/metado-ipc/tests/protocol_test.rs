//! Task 5.3: 管理面协议定义（§11）——方法名、载荷形状、dispatch 契约。

use metado_ipc::jsonrpc::{ErrorObject, Request, Response};
use metado_ipc::protocol::{handle_one, InvokeParams, Params, ProtocolError};

#[test]
fn test_load_plugin_params_schema() {
    let p = Params::LoadPlugin {
        file_hex: "ab0012".to_string(),
        permits: vec!["storage.read".to_string(), "log.info".to_string()],
    };
    assert_eq!(p.method_name(), "loadPlugin");
    let v = p.to_value();
    assert_eq!(v["file"], "ab0012");
    assert_eq!(v["permits"][1], "log.info");
    let back = Params::from_method("loadPlugin", &v).unwrap();
    match back {
        Params::LoadPlugin { file_hex, permits } => {
            assert_eq!(file_hex, "ab0012");
            assert_eq!(permits.len(), 2);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_invoke_params_roundtrip() {
    let p = Params::Invoke(InvokeParams {
        entry: "entry:boot".into(),
        args: serde_json::json!([1, 2]),
    });
    assert_eq!(p.method_name(), "invoke");
    let v = p.to_value();
    assert_eq!(v["entry"], "entry:boot");
    match Params::from_method("invoke", &v).unwrap() {
        Params::Invoke(i) => assert_eq!(i.entry, "entry:boot"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn test_every_management_method_parses() {
    let cases: Vec<(&str, serde_json::Value)> = vec![
        ("grant", serde_json::json!({ "plugin_id": "p1", "permission": ["log.info"] })),
        ("revoke", serde_json::json!({ "plugin_id": "p1" })),
        ("setLifecycle", serde_json::json!({ "plugin_id": "p1", "state": "running" })),
        ("listPlugins", serde_json::json!({})),
        ("uninstall", serde_json::json!({ "plugin_id": "p1" })),
        ("purge", serde_json::json!({})),
        ("registerPermissionSet", serde_json::json!({ "name": "standard", "permissions": ["log.info"] })),
        ("setDomainConfig", serde_json::json!({ "domain": "example.com", "config": { "allow": true } })),
        ("setActive", serde_json::json!({ "plugin_id": "p1", "active": true })),
        ("trace", serde_json::json!({ "plugin_id": "p1" })),
    ];
    for (method, params) in &cases {
        let p = Params::from_method(method, params).unwrap_or_else(|e| panic!("{}: {}", method, e));
        assert_eq!(p.method_name(), *method, "method_name must invert for {}", method);
    }
}

#[test]
fn test_unknown_method_and_bad_params() {
    assert!(matches!(
        Params::from_method("doesNotExist", &serde_json::json!({})),
        Err(ProtocolError::UnknownMethod(_))
    ));
    assert!(matches!(
        Params::from_method("grant", &serde_json::json!({ "plugin_id": 4 })),
        Err(ProtocolError::BadParams(_))
    ));
    assert_eq!(ProtocolError::UnknownMethod("x".into()).message(), "unknown method x");
}

#[test]
fn test_handle_one_dispatches_request_to_method() {
    let req = Request::named("listPlugins").with_params(serde_json::json!({})).with_id(5u64);
    let resp = handle_one(&req, &mut |m, _p| {
        assert_eq!(m, "listPlugins");
        Ok(serde_json::json!({ "plugins": [] }))
    })
    .unwrap()
    .expect("request must get a reply");
    assert_eq!(resp.id, serde_json::json!(5));
    assert_eq!(resp.result.unwrap()["plugins"], serde_json::json!([]));
}

#[test]
fn test_handle_one_unknown_method_yields_error_response() {
    let req = Request::named("bogus").with_params(serde_json::json!({})).with_id(1u64);
    let resp: Response = handle_one(&req, &mut |_m, _p| Ok(serde_json::json!(null)))
        .unwrap()
        .unwrap();
    assert!(resp.is_error());
    let err = resp.error.unwrap();
    assert_eq!(err.code, -32601);
}

#[test]
fn test_handle_one_notification_suppresses_reply() {
    let notif = Request::notification("dispatch", serde_json::json!(["onMessage", {}]));
    let out = handle_one(&notif, &mut |m, _p| {
        assert_eq!(m, "dispatch");
        Ok(serde_json::json!(null))
    })
    .unwrap();
    assert!(out.is_none(), "notifications must produce no reply");
}

#[test]
fn test_handler_error_maps_to_error_object() {
    let req = Request::named("invoke").with_params(serde_json::json!({ "entry": "x" })).with_id(2u64);
    let resp = handle_one(&req, &mut |_m, _p| {
        Err(ProtocolError::Dispatch("entry not found".into()))
    })
    .unwrap()
    .unwrap();
    let e = resp.error.unwrap();
    assert_eq!(e.code, -32603);
    assert!(e.message.contains("entry not found"));
}

#[test]
fn test_error_object_roundtrip_from_internal_error() {
    let eo = ErrorObject::from(ProtocolError::UnknownMethod("frob".into()));
    assert_eq!(eo.code, -32601);
    assert!(eo.message.contains("frob"));
}