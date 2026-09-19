//! Task 1.10: Engine facade integration tests (§13)

use std::io::Write;

use metado_engine::{signer_id, Engine, KeyPair, SignedBundle, Value};

fn build_signed_bundle(toml: &str, extra_files: &[(&str, &str)]) -> (SignedBundle, KeyPair) {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("mdl.toml", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(toml.as_bytes()).unwrap();
    for (name, content) in extra_files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    let data = zip.finish().unwrap().into_inner();

    let kp = KeyPair::generate();
    let bundle = SignedBundle::sign(&kp, &data);
    (bundle, kp)
}

#[test]
fn test_engine_create_and_register() {
    let mut engine = Engine::new();
    engine.define_permission_set("standard", vec!["http.get".into()]);
    // Engine 创建成功
    let _ = engine;
}

#[test]
fn test_plugin_load_grant_activate_invoke() {
    let (bundle, kp) = build_signed_bundle(
        r#"
            name = "myplugin"
            version = "1.0.0"
        "#,
        &[("src/main.js", "export function boot() { return 1; }")],
    );

    let mut engine = Engine::new();
    engine.load_plugin(&bundle, vec!["http.get".into()]).unwrap();

    {
        let plugin = engine.plugin("myplugin").unwrap();
        assert_eq!(plugin.id, "myplugin");
        assert_eq!(plugin.signer_id, signer_id(&kp.public_key_bytes()));
        assert!(plugin.granted.contains(&"http.get".to_string()));
        assert_eq!(plugin.manifest.as_ref().unwrap().version, "1.0.0");
        // 未激活前 invoke 拒绝
        assert!(engine.invoke("myplugin", "boot", Value::Null).is_err());
    }

    engine.grant("myplugin", vec!["log.info".into()]).unwrap();
    assert!(engine.plugin("myplugin").unwrap().granted.iter().any(|p| p == "log.info"));

    engine.activate("myplugin").unwrap();

    let out = engine.invoke("myplugin", "boot", Value::String("hi".into()));
    let out = out.unwrap();
    assert!(matches!(out, Value::String(s) if s == "hi")); // v1 stub 回传
}

#[test]
fn test_load_plugin_rejects_bad_signature() {
    let (mut bundle, _kp) = build_signed_bundle(
        r#"
            name = "bad"
            version = "1.0.0"
        "#,
        &[],
    );
    // 篡改 payload 破坏签名
    let len = bundle.payload().len();
    bundle.payload_mut()[len - 1] ^= 0xFF;

    let mut engine = Engine::new();
    assert!(engine.load_plugin(&bundle, Vec::new()).is_err());
}

#[test]
fn test_invoke_unknown_plugin() {
    let engine = Engine::new();
    let err = engine.invoke("nope", "boot", Value::Null).unwrap_err();
    assert_eq!(err.kind, metado_engine::ErrorKind::PluginError);
}