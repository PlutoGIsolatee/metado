//! Task 5.6: Daemon —— engine 生命周期 + PluginRuntime 执行 + IPC 分发。
//! 覆盖：load/revoke/list/uninstall/invoke/grant/setActive/setDomainConfig/
//! registerPermissionSet/trace + Unix 全栈 roundtrip。

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use metado_daemon::Daemon;
use metado_engine::{KeyPair, SignedBundle};
use metado_ipc::jsonrpc::{Request as RpcReq, Response};
use metado_ipc::transport::Transport;
use metado_ipc::UnixTransport;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unique_sock(tag: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut p = std::env::temp_dir();
    p.push(format!("metado-daemon-{}-{}-{}.sock", tag, std::process::id(), n));
    p
}

/// 构造 .mdl 字节（zip: mdl.toml + src/main.js，签名）。
fn make_bundle(plugin_id: &str, main_js: &str) -> SignedBundle {
    let manifest = format!(
        "name = \"{id}\"\nversion = \"1.0.0\"\npermission = [\"log.info\"]\n[entries.boot]\nexport = \"boot\"\n",
        id = plugin_id
    );
    make_bundle_toml(plugin_id, &manifest, main_js)
}

fn make_bundle_toml(_plugin_id: &str, manifest: &str, main_js: &str) -> SignedBundle {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cursor);
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file("mdl.toml", opts).unwrap();
    zw.write_all(manifest.as_bytes()).unwrap();
    zw.start_file("src/main.js", opts).unwrap();
    zw.write_all(main_js.as_bytes()).unwrap();
    let cursor = zw.finish().unwrap();
    let kp = KeyPair::generate();
    SignedBundle::sign(&kp, &cursor.into_inner())
}

fn hex_str(bytes: &[u8]) -> String {
    hex::encode(bytes)
}



#[test]
fn test_daemon_load_and_invoke_flow() {
    let bundle = make_bundle("greeter", "export default { boot() { return 'ahoy'; } };\n");
    let mut daemon = Daemon::new();
    let v = daemon
        .method("loadPlugin", &serde_json::json!({ "file": hex_str(&bundle.to_bytes()), "permits": ["log.info"] }))
        .unwrap();
    assert_eq!(v["plugin_id"], "greeter");

    let listed = daemon.method("listPlugins", &serde_json::json!({})).unwrap();
    assert_eq!(listed[0]["name"], "greeter");

    let out = daemon
        .method("invoke", &serde_json::json!({ "entry": "greeter:boot", "args": [] }))
        .unwrap();
    assert_eq!(out, serde_json::json!("ahoy"));
}

#[test]
fn test_daemon_revokes_then_invoke_fails() {
    let bundle = make_bundle("one", "export default { boot() { return 1; } };\n");
    let mut daemon = Daemon::new();
    daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bundle.to_bytes()) })).unwrap();
    daemon.method("revoke", &serde_json::json!({ "plugin_id": "one" })).unwrap();
    let err = daemon
        .method("invoke", &serde_json::json!({ "entry": "one:boot", "args": [] }))
        .unwrap_err();
    assert!(err.contains("one"), "got {}", err);
}

#[test]
fn test_daemon_uninstall_and_purge() {
    let mut daemon = Daemon::new();
    for n in 0..2 {
        let bundle = make_bundle(&format!("p{}", n), "export default { boot() { return 0; } };\n");
        daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bundle.to_bytes()) })).unwrap();
    }
    daemon.method("uninstall", &serde_json::json!({ "plugin_id": "p0" })).unwrap();
    let listed = daemon.method("listPlugins", &serde_json::json!({})).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    daemon.method("purge", &serde_json::json!({})).unwrap();
    let listed = daemon.method("listPlugins", &serde_json::json!({})).unwrap();
    assert!(listed.as_array().unwrap().is_empty());
}

#[test]
fn test_daemon_permission_set_and_domain_config() {
    let mut daemon = Daemon::new();
    let v = daemon
        .method("registerPermissionSet", &serde_json::json!({ "name": "chatty", "permissions": ["log.info", "http.fetch"] }))
        .unwrap();
    assert_eq!(v["name"], "chatty");
    assert_eq!(v["permissions"][1], "http.fetch");

    daemon
        .method("setDomainConfig", &serde_json::json!({ "domain": "example.com", "config": { "allow": true } }))
        .unwrap();
    // v1 不持久化域配置：仅登记
    let err = daemon
        .method("setDomainConfig", &serde_json::json!({ "config": {} }))
        .unwrap_err();
    assert!(err.contains("missing string param domain"), "got {}", err);
}

#[test]
fn test_daemon_grant_set_active_trace() {
    let bundle = make_bundle(
        "spy",
        "import { storage } from '@metado/runtime';\nexport default { boot() { return typeof storage; } };\n",
    );
    let mut daemon = Daemon::new();
    daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bundle.to_bytes()), "permits": ["storage.read"] })).unwrap();
    daemon.method("grant", &serde_json::json!({ "plugin_id": "spy", "permission": ["storage.write"] })).unwrap();
    daemon.method("setActive", &serde_json::json!({ "plugin_id": "spy", "active": true })).unwrap();
    daemon.method("invoke", &serde_json::json!({ "entry": "spy:boot", "args": [] })).unwrap();

    let tr = daemon.method("trace", &serde_json::json!({ "plugin_id": "spy" })).unwrap();
    assert!(!tr.as_array().unwrap().is_empty(), "trace must record events");
}

#[test]
fn test_daemon_grant_filtered_beyond_requested() {
    // 声明 log.info；load 附加 storage.read。storage.write 越界 → GRANT 被过滤
    let bundle = make_bundle("grp", "export default { boot() { return 1; } };\n");
    let mut daemon = Daemon::new();
    daemon
        .method("loadPlugin", &serde_json::json!({ "file": hex_str(&bundle.to_bytes()), "permits": ["storage.read"] }))
        .unwrap();
    let v = daemon
        .method("grant", &serde_json::json!({ "plugin_id": "grp", "permission": ["storage.write", "log.info", "storage.read"] }))
        .unwrap();
    let granted: Vec<String> = v["granted"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect();
    assert!(granted.contains(&"log.info".to_string()));
    assert!(granted.contains(&"storage.read".to_string()));
    assert!(!granted.contains(&"storage.write".to_string()), "out-of-scope grant must be filtered, got {:?}", granted);

    // engine 权威 granted 一致
    let list = daemon.method("listPlugins", &serde_json::json!({})).unwrap();
    let me = list
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "grp")
        .unwrap();
    let listed: Vec<String> = me["granted"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect();
    assert_eq!(listed, granted);
}

#[test]
fn test_daemon_load_expands_registered_permission_set() {
    daemon_register_set_and_grants_expand(true)
}

#[test]
fn test_daemon_load_rejects_undefined_permission_set() {
    daemon_register_set_and_grants_expand(false)
}

fn daemon_register_set_and_grants_expand(register: bool) {
    let manifest = "name = \"pset\"\nversion = \"1.0.0\"\npermission-set = [\"chatty\"]\n[entries.boot]\nexport = \"boot\"\n";
    let bundle = make_bundle_toml("pset", manifest, "export default { boot() { return 1; } };\n");
    let mut daemon = Daemon::new();
    if register {
        daemon
            .method("registerPermissionSet", &serde_json::json!({ "name": "chatty", "permissions": ["log.info", "http.fetch"] }))
            .unwrap();
    }
    let res = daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bundle.to_bytes()) }));
    if register {
        let list = daemon.method("listPlugins", &serde_json::json!({})).unwrap();
        let me = list
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "pset")
            .unwrap();
        let granted: Vec<String> = me["granted"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
        assert_eq!(granted, vec!["log.info", "http.fetch"]);
        let _ = res.unwrap();
    } else {
        let err = res.unwrap_err();
        assert!(err.contains("chatty"), "undefined permission set must fail load, got {}", err);
    }
}

#[test]
fn test_daemon_method_error_maps() {
    let mut daemon = Daemon::new();
    let err = daemon.method("invoke", &serde_json::json!({ "entry": "nope:boot", "args": [] })).unwrap_err();
    assert!(err.contains("nope"), "got {}", err);
}

#[test]
fn test_daemon_unix_full_stack_roundtrip() {
    let bundle = make_bundle("ipc", "export default { boot() { return 'over-ipc'; } };\n");
    let path = unique_sock("stack");
    let server_path = path.clone();

    let server = std::thread::spawn(move || {
        let mut daemon = Daemon::new();
        daemon.serve_unix(&server_path).unwrap();
    });
    std::thread::sleep(std::time::Duration::from_millis(150));

    let mut client = UnixTransport::connect(&path).unwrap();
    let req = RpcReq::named("loadPlugin")
        .with_params(serde_json::json!({ "file": hex_str(&bundle.to_bytes()) }))
        .with_id(1u64);
    client.send(&req.to_json().to_string().into_bytes()).unwrap();
    let payload = client.receive().unwrap();
    let resp = Response::from_json(&String::from_utf8(payload).unwrap()).unwrap();
    assert!(!resp.is_error(), "loadPlugin over IPC failed: {:?}", resp.error);
    assert_eq!(resp.result.clone().unwrap()["plugin_id"], "ipc");

    let req = RpcReq::named("invoke")
        .with_params(serde_json::json!({ "entry": "ipc:boot", "args": [] }))
        .with_id(2u64);
    client.send(&req.to_json().to_string().into_bytes()).unwrap();
    let payload = client.receive().unwrap();
    let resp = Response::from_json(&String::from_utf8(payload).unwrap()).unwrap();
    assert_eq!(resp.result.clone().unwrap(), serde_json::json!("over-ipc"));
    client.close();

    let _ = std::fs::remove_file(&path);
    let _ = server; // detach; daemon threads end with process exit
}