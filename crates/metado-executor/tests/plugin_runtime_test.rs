//! PluginRuntime（boa ESM）集成测试：容器 import、@metado/runtime 合成模块、default 方法调用。

use std::collections::HashMap;

use metado_engine::Value;
use metado_executor::{FilesFn, PluginRuntime, runtime_namespaces};

fn files_of(map: HashMap<String, Vec<u8>>) -> FilesFn {
    Box::new(move |rel: &str| map.get(rel).cloned())
}

fn stub(map: HashMap<&str, &str>) -> FilesFn {
    let m = map
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
        .collect();
    files_of(m)
}

#[test]
fn test_load_and_call_default() {
    let files = stub(HashMap::from([
        ("src/main.js", "export default { boot() { return 41 + 1; } };\n"),
    ]));
    let mut rt = PluginRuntime::new(&[], &[], files).unwrap();
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out.as_f64(), Some(42.0));
}

#[test]
fn test_relative_import_resolves_in_container() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { helper } from './helper.js';\nexport default { boot() { return helper(); } };\n",
        ),
        ("src/helper.js", "export function helper() { return 'hi'; }\n"),
    ]));
    let mut rt = PluginRuntime::new(&[], &[], files).unwrap();
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    match out {
        Value::String(s) => assert_eq!(s, "hi"),
        other => panic!("expected string, got {:?}", other),
    }
}

#[test]
fn test_runtime_export_present_when_available() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { storage } from '@metado/runtime';\nexport default { boot() { return typeof storage; } };\n",
        ),
    ]));
    let available = vec!["storage.read".to_string(), "storage.write".to_string()];
    let mut rt = PluginRuntime::new(&available, &[], files).unwrap();
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("function".into()));
}

#[test]
fn test_runtime_export_absent_when_not_available() {
    // §4.3：未请求的能力导出不存在 → import 未 available 的命名空间应链接失败
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { http } from '@metado/runtime';\nexport default { boot() { return 1; } };\n",
        ),
    ]));
    let available = vec!["log.info".to_string()];
    let mut rt = PluginRuntime::new(&available, &[], files).unwrap();
    assert!(rt.load("src/main.js").is_err(), "importing ungranted namespace must fail to link");
}

#[test]
fn test_args_pass_through() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "export default { greet(name) { return 'hello ' + name; } };\n",
        ),
    ]));
    let mut rt = PluginRuntime::new(&[], &[], files).unwrap();
    rt.load("src/main.js").unwrap();
    let out = rt
        .call_default("greet", vec![Value::String("metado".into())])
        .unwrap();
    assert_eq!(out, Value::String("hello metado".into()));
}

#[test]
fn test_throwing_export_propagates() {
    let files = stub(HashMap::from([
        ("src/main.js", "export default { boom() { throw new Error('kaput'); } };\n"),
    ]));
    let mut rt = PluginRuntime::new(&[], &[], files).unwrap();
    rt.load("src/main.js").unwrap();
    assert!(rt.call_default("boom", vec![]).is_err());
}

#[test]
fn test_runtime_namespaces_include_capabilities_and_metado() {
    let available = vec![
        "http.get".to_string(),
        "storage.<signer>.read".to_string(),
    ];
    let names = runtime_namespaces(&available);
    for expected in ["http", "storage", "metado"] {
        assert!(names.contains(&expected.to_string()), "missing {}", expected);
    }
}