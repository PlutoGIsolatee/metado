//! Task 6.4 契约：CLI 与 daemon 生产行为对齐（同插件同授权 → 同结果）。

use metado_contract::common::{bundle_bytes, hex_str};
use metado_cli::{run_mdl, test_mdl};
use metado_daemon::Daemon;

#[test]
fn contract_daemon_invoke_matches_cli_run() {
    let bytes = bundle_bytes(
        "name = \"align\"\nversion = \"1.0.0\"\npermission = [\"storage.read\", \"log.info\"]\n[entries.boot]\nexport = \"boot\"\n",
        "import { storage } from '@metado/runtime';\nexport default { boot() { return typeof storage; } };\n",
    );

    let cli_run = run_mdl(&bytes, &[]).unwrap();
    assert_eq!(cli_run.result, metado_engine::Value::String("function".to_string()));

    let mut daemon = Daemon::new();
    daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bytes) })).unwrap();
    let out = daemon.method("invoke", &serde_json::json!({ "entry": "align:boot", "args": [] })).unwrap();
    assert_eq!(out, serde_json::json!("function"));
}

#[test]
fn contract_daemon_load_error_matches_cli() {
    // 篡改后的 bundle：CLI run 与 daemon load 都拒绝
    let mut bytes = bundle_bytes(
        "name = \"tamper\"\nversion = \"1.0.0\"\n",
        "export default {};\n",
    );
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;

    assert!(run_mdl(&bytes, &[]).is_err());
    let mut daemon = Daemon::new();
    let err = daemon
        .method("loadPlugin", &serde_json::json!({ "file": hex_str(&bytes) }))
        .unwrap_err();
    assert!(!err.is_empty(), "daemon must surface a load error");
}

#[test]
fn contract_daemon_revoke_makes_invoke_fail_like_cli_ungranted() {
    let bytes = bundle_bytes(
        "name = \"d\"\nversion = \"1.0.0\"\npermission = [\"http.get\"]\n[entries.boot]\nexport = \"boot\"\n",
        "export default { boot() { return 'x'; } };\n",
    );
    let mut daemon = Daemon::new();
    daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bytes) })).unwrap();
    daemon.method("revoke", &serde_json::json!({ "plugin_id": "d" })).unwrap();
    let err = daemon.method("invoke", &serde_json::json!({ "entry": "d:boot", "args": [] })).unwrap_err();
    assert!(err.contains("d"));
}

#[test]
fn contract_daemon_list_plugins_matches_manifest() {
    let bytes = bundle_bytes(
        "name = \"listed\"\nversion = \"1.0.0\"\npermission = [\"log.info\"]\n[entries.boot]\nexport = \"boot\"\n[entries.onMessage]\nexport = \"onMessage\"\n",
        "export default { boot() { return 0; }, onMessage() { return 0; } };\n",
    );
    let mut daemon = Daemon::new();
    daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bytes) })).unwrap();
    let listed = daemon.method("listPlugins", &serde_json::json!({})).unwrap();
    let first = &listed[0];
    assert_eq!(first["name"], "listed");
    assert_eq!(first["entries"], serde_json::json!(["boot", "onMessage"]));
}

#[test]
fn contract_test_cmd_matches_daemon_trace_lifecycle() {
    let bytes = bundle_bytes(
        "name = \"life\"\nversion = \"1.0.0\"\npermission = [\"log.info\"]\n[entries.boot]\nexport = \"boot\"\n[entries.test]\nexport = \"test\"\n",
        "export default { boot() { return 1; }, test() { return true; } };\n",
    );
    // mdl test 语义：声明 entries.test 则只跑 test 用例（test 优先于 boot 回退）
    let outcomes = test_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].name, "test");
    assert!(outcomes[0].passed);

    let mut daemon = Daemon::new();
    daemon.method("loadPlugin", &serde_json::json!({ "file": hex_str(&bytes) })).unwrap();
    let tr = daemon.method("trace", &serde_json::json!({ "plugin_id": "life" })).unwrap();
    let kinds: Vec<&str> = tr
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.get("kind").and_then(|k| k.as_str()).unwrap_or(""))
        .collect();
    assert!(kinds.contains(&"EntryStart") && kinds.contains(&"EntryEnd"));
}