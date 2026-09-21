//! 评审修复 C1/C2：权限治理接线测试。
//! C1: granted ⊆ requested 强制（越界 grant 被过滤；模板内细粒度授权保留）+ permission-set 展开/存在性。
//! C2: load_plugin 同 id 闸门——同 signer 替换、不同 signer 拒绝（防冒名）。

use std::io::Write;

use metado_engine::{Engine, KeyPair, SignedBundle};

fn build_signed_bundle(toml: &str) -> (SignedBundle, KeyPair) {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("mdl.toml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(toml.as_bytes()).unwrap();
    zip.start_file("src/main.js", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"export function boot() { return 0; }").unwrap();
    let data = zip.finish().unwrap().into_inner();
    let kp = KeyPair::generate();
    let bundle = SignedBundle::sign(&kp, &data);
    (bundle, kp)
}

#[test]
fn test_grant_beyond_requested_is_filtered() {
    let (bundle, _kp) = build_signed_bundle(
        r#"
            name = "grp"
            version = "1.0.0"
            permission = ["http.get"]
        "#,
    );
    let mut engine = Engine::new();
    engine.load_plugin(&bundle, vec!["http.get".into()]).unwrap();

    // 越界 grant（manifest 未请求）被拒绝：storage.write 不生效，http.get 保留
    let granted = engine.grant("grp", vec!["storage.write".into(), "http.get".into()]).unwrap();
    assert_eq!(granted, vec!["http.get"]);
    assert_eq!(engine.granted("grp").unwrap(), vec!["http.get"]);
}

#[test]
fn test_grant_inside_requested_template_kept() {
    let (bundle, _kp) = build_signed_bundle(
        r#"
            name = "tpl"
            version = "1.0.0"
            permission = ["http.get.api.*"]
        "#,
    );
    let mut engine = Engine::new();
    engine.load_plugin(&bundle, vec!["http.get.api.*".into()]).unwrap();

    // 模板覆盖内的细粒度授权有效：域名/父方法（load 已 seed granted=requested 模板本身）
    let granted = engine
        .grant("tpl", vec!["http.get.api.example.com".into(), "http.get".into()])
        .unwrap();
    assert_eq!(
        granted,
        vec!["http.get.api.*", "http.get.api.example.com", "http.get"]
    );
}

#[test]
fn test_grant_dedups() {
    let (bundle, _kp) = build_signed_bundle(
        r#"
            name = "dd"
            version = "1.0.0"
            permission = ["log.info"]
        "#,
    );
    let mut engine = Engine::new();
    engine.load_plugin(&bundle, vec!["log.info".into()]).unwrap();
    let granted = engine.grant("dd", vec!["log.info".into(), "log.info".into()]).unwrap();
    assert_eq!(granted, vec!["log.info"]);
}

#[test]
fn test_load_records_requested() {
    let (bundle, _kp) = build_signed_bundle(
        r#"
            name = "req"
            version = "1.0.0"
            permission = ["log.info"]
        "#,
    );
    let mut engine = Engine::new();
    engine.load_plugin(&bundle, vec!["log.info".into()]).unwrap();
    assert_eq!(engine.requested("req").unwrap(), vec!["log.info"]);
    assert_eq!(engine.granted("req").unwrap(), vec!["log.info"]);
}

#[test]
fn test_load_expands_permission_set_into_requested() {
    let (bundle, _kp) = build_signed_bundle(
        r#"
            name = "px"
            version = "1.0.0"
            permission = ["log.info"]
            permission-set = ["standard"]
        "#,
    );
    let mut engine = Engine::new();
    engine.define_permission_set("standard", vec!["http.get".into(), "storage.read".into()]);
    engine.load_plugin(&bundle, vec!["log.info".into()]).unwrap();

    // requested = 声明 ∪ set 展开；加载即按 requested 授予（调用面）
    let mut req = engine.requested("px").unwrap();
    req.sort();
    assert_eq!(
        req,
        vec!["http.get", "log.info", "storage.read"].iter().map(|s| s.to_string()).collect::<Vec<_>>()
    );
    assert_eq!(engine.granted("px").unwrap().len(), 3);
}

#[test]
fn test_load_rejects_undefined_permission_set() {
    let (bundle, _kp) = build_signed_bundle(
        r#"
            name = "badset"
            version = "1.0.0"
            permission-set = ["ghost"]
        "#,
    );
    let mut engine = Engine::new();
    let err = engine.load_plugin(&bundle, Vec::new()).unwrap_err();
    assert!(err.contains("ghost"), "expected undefined set error, got {}", err);
    assert!(engine.plugin("badset").is_err(), "failed load must not register plugin");
}

#[test]
fn test_reload_same_signer_replaces() {
    let (bundle, _kp) = build_signed_bundle(
        r#"
            name = "up"
            version = "2.0.0"
            permission = ["log.info"]
        "#,
    );
    let mut engine = Engine::new();
    engine.load_plugin(&bundle, vec!["log.info".into()]).unwrap();
    // 同 signer 重新加载 → 更新（version 2.0.0），不新增重复记录
    engine.load_plugin(&bundle, vec!["log.info".into()]).unwrap();
    let p = engine.plugin("up").unwrap();
    assert_eq!(p.manifest.as_ref().unwrap().version, "2.0.0");
    // plugin()/grant() 应命中最新记录（旧记录已移除）
    assert!(engine.grant("up", vec!["log.info".into()]).is_ok());
}

#[test]
fn test_reload_different_signer_rejected() {
    let (bundle, kp) = build_signed_bundle(
        r#"
            name = "locked"
            version = "1.0.0"
        "#,
    );
    let mut engine = Engine::new();
    engine.load_plugin(&bundle, Vec::new()).unwrap();

    // 不同密钥重签同名插件 → 拒绝（防冒名），原插件保持
    let kp2 = KeyPair::generate();
    let bundle2 = SignedBundle::sign(&kp2, bundle.payload());
    let err = engine.load_plugin(&bundle2, Vec::new()).unwrap_err();
    assert!(err.contains("locked"), "expected conflict error, got {}", err);
    assert!(engine.plugin("locked").is_ok(), "original plugin must survive");
    let _ = kp;
}