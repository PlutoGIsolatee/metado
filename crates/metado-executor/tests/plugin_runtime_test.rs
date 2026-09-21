//! PluginRuntime（boa ESM）集成测试：容器 import、@metado/runtime 合成模块、
//! 命名空间对象形状、方法级放行（异步 reject / 同步 throw）、默认方法调用（含 async 入口）。

use std::collections::HashMap;

use metado_engine::{exported_namespaces, TraceEvent, Value};
use metado_executor::{FilesFn, PluginRuntime};

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

fn new_runtime(exported: &[String], granted: &[String], surface: Vec<String>, files: FilesFn) -> PluginRuntime {
    PluginRuntime::new(exported, granted, &surface, files).unwrap()
}

/// 常见方法面：与真实宿主注册集对齐。
fn host_surface() -> Vec<String> {
    vec![
        "http.get".into(),
        "http.post".into(),
        "http.get.api.*".into(),
        "storage.read".into(),
        "storage.write".into(),
        "file.read".into(),
        "file.stat".into(),
        "time.now".into(),
        "time.sleep".into(),
        "log.info".into(),
        "log.warn".into(),
        "log.error".into(),
        "log.debug".into(),
        "crypto.randomBytes".into(),
        "crypto.sha256".into(),
        "crypto.hmac".into(),
    ]
}

#[test]
fn test_load_and_call_default() {
    let files = stub(HashMap::from([
        ("src/main.js", "export default { boot() { return 41 + 1; } };\n"),
    ]));
    let mut rt = new_runtime(&[], &[], host_surface(), files);
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
    let mut rt = new_runtime(&[], &[], host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    match out {
        Value::String(s) => assert_eq!(s, "hi"),
        other => panic!("expected string, got {:?}", other),
    }
}

#[test]
fn test_runtime_export_is_namespace_object_with_methods() {
    // C5：导出形状 = 命名空间对象，方法为函数（http.get(url) 而非裸函数 stub）
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { storage } from '@metado/runtime';\n\
             export default { boot() { return typeof storage + '/' + typeof storage.read + '/' + typeof storage.write; } };\n",
        ),
    ]));
    let exported = vec!["storage".into()];
    let granted = vec!["storage.read".into(), "storage.write".into()];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("object/function/function".into()));
}

#[test]
fn test_signer_template_surface_yields_bare_methods() {
    // 审计 3.2：surface/granted 仅含 <signer> 模板形式（无裸 read/write）时，
    // storage 的方法面仍应为 read/write，而非字面 "<signer>" 假方法。
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { storage } from '@metado/runtime';\n\
             export default { boot() { return Object.keys(storage).join(',') + '/' + typeof storage.read + '/' + typeof storage.write; } };\n",
        ),
    ]));
    let exported = vec!["storage".into()];
    let granted = vec!["storage.<signer>.read".into(), "storage.<signer>.write".into()];
    let surface = vec![
        "storage.<signer>.read".to_string(),
        "storage.<signer>.write".to_string(),
        "log.info".to_string(),
    ];
    let mut rt = new_runtime(&exported, &granted, surface, files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("read,write/function/function".into()));
}

#[test]
fn test_runtime_neutral_namespaces_are_objects() {
    // 请求了但未授予的能力面：命名空间对象仍在，方法仍存在（放行是调用期动态面）
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { http } from '@metado/runtime';\n\
             export default { boot() { return typeof http + '/' + typeof http.get + '/' + typeof http.post; } };\n",
        ),
    ]));
    let exported = vec!["http".into()];
    let granted = vec!["log.info".into()];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("object/function/function".into()));
}

#[test]
fn test_runtime_export_absent_when_not_exported() {
    // §4.3：未请求的能力导出不存在 → import 未在 exported 中的命名空间应链接失败
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { http } from '@metado/runtime';\nexport default { boot() { return 1; } };\n",
        ),
    ]));
    let exported = vec!["log".into()];
    let granted = vec!["log.info".into()];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    assert!(rt.load("src/main.js").is_err(), "importing unexported namespace must fail to link");
}

#[test]
fn test_runtime_export_is_namespace_level() {
    // C5 回归：请求 http.get.api.example（可用 http.get.api.*）→ http 命名空间仍导出
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { http } from '@metado/runtime';\nexport default { boot() { return typeof http.get; } };\n",
        ),
    ]));
    let requested = vec!["http.get.api.example".into()];
    let available = host_surface();
    let exported = exported_namespaces(&requested, &available);
    assert!(exported.contains(&"http".to_string()), "http must stay exported: {:?}", exported);
    let granted = vec!["http.get.api.*".into()];
    let mut rt = new_runtime(&exported, &granted, available, files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("function".into()));
}

#[test]
fn test_metado_always_exported() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { metado } from '@metado/runtime';\n\
             export default { boot() { return typeof metado + '/' + typeof metado.custom; } };\n",
        ),
    ]));
    let mut rt = new_runtime(&[], &[], host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("object/function".into()));
}

#[test]
fn test_args_pass_through() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "export default { greet(name) { return 'hello ' + name; } };\n",
        ),
    ]));
    let mut rt = new_runtime(&[], &[], host_surface(), files);
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
    let mut rt = new_runtime(&[], &[], host_surface(), files);
    rt.load("src/main.js").unwrap();
    assert!(rt.call_default("boom", vec![]).is_err());
}

#[test]
fn test_async_entry_pumps_jobs_and_reads_state() {
    // C4/Important：异步入口真实返回（async + await 需 pump job 队列并读 Promise 状态）
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "export default { boot: async () => { const v = await Promise.resolve(42); return v; } };\n",
        ),
    ]));
    let mut rt = new_runtime(&[], &[], host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out.as_f64(), Some(42.0));
}

#[test]
fn test_async_entry_rejects_propagate() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "export default { boot: async () => { throw new Error('nope'); } };\n",
        ),
    ]));
    let mut rt = new_runtime(&[], &[], host_surface(), files);
    rt.load("src/main.js").unwrap();
    assert!(rt.call_default("boot", vec![]).is_err());
}

#[test]
fn test_denied_async_capability_rejects_promise() {
    // C4：异步形状未授权 → Promise.reject(PermissionDenied)，插件可 catch 但拿不到 stub 值
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { storage } from '@metado/runtime';\n\
             export default { boot: async () => { try { await storage.write('k'); return 'allowed'; } catch (e) { return 'denied'; } } };\n",
        ),
    ]));
    let exported = vec!["storage".into()];
    let granted = vec!["storage.read".into()];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("denied".into()));
}

#[test]
fn test_granted_async_capability_resolves_stub() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { storage } from '@metado/runtime';\n\
             export default { boot: async () => { await storage.write('k'); return 'allowed'; } };\n",
        ),
    ]));
    let exported = vec!["storage".into()];
    let granted = vec!["storage.read".into(), "storage.write".into()];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("allowed".into()));
}

#[test]
fn test_denied_sync_capability_throws() {
    // C4：同步形状（log）未授权 → 同步 throw → 入口 Err
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { log } from '@metado/runtime';\n\
             export default { boot() { log.info('x'); return 'after'; } };\n",
        ),
    ]));
    let exported = vec!["log".into()];
    let granted = vec![];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    rt.load("src/main.js").unwrap();
    assert!(rt.call_default("boot", vec![]).is_err(), "denied sync log must throw");
}

#[test]
fn test_granted_sync_capability_runs() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { log } from '@metado/runtime';\n\
             export default { boot() { log.info('x'); return 'ok'; } };\n",
        ),
    ]));
    let exported = vec!["log".into()];
    let granted = vec!["log.info".into()];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    rt.load("src/main.js").unwrap();
    let out = rt.call_default("boot", vec![]).unwrap();
    assert_eq!(out, Value::String("ok".into()));
}

#[test]
fn test_trace_emits_capability_and_permission() {
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "import { storage } from '@metado/runtime';\nexport default { boot() { return 1; } };\n",
        ),
    ]));
    let exported = vec!["storage".into()];
    let granted = vec!["storage.read".into(), "storage.write".into()];
    let mut rt = new_runtime(&exported, &granted, host_surface(), files);
    let events: std::rc::Rc<std::cell::RefCell<Vec<TraceEvent>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = events.clone();
    rt.set_trace(Box::new(move |ev| sink.borrow_mut().push(ev)));
    rt.load("src/main.js").unwrap();

    let calls: Vec<String> = events
        .borrow()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::CapabilityCall { capability, .. } => Some(capability.clone()),
            _ => None,
        })
        .collect();
    // granted 字段如实传入（不再谎报为导出名）
    let granted_seen: Vec<Vec<String>> = events
        .borrow()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::CapabilityCall { granted, .. } => Some(granted.clone()),
            _ => None,
        })
        .collect();
    assert!(calls.contains(&"storage.read".to_string()));
    assert!(calls.contains(&"storage.write".to_string()));
    assert!(calls.iter().any(|c| c == "metado.custom"));
    assert!(granted_seen.iter().all(|g| g == &granted));
    let checks: Vec<(String, bool)> = events
        .borrow()
        .iter()
        .filter_map(|e| match e {
            TraceEvent::PermissionCheck { capability, passed } => Some((capability.clone(), *passed)),
            _ => None,
        })
        .collect();
    assert!(checks.contains(&("storage.read".to_string(), true)));
    assert!(checks.contains(&("storage.write".to_string(), true)));
}

#[test]
fn test_infinite_loop_hits_instruction_budget() {
    // C3：指令燃料 —— 紧循环（无 stub 交互，ExecutionBudget 看不见）仍被终结为 Err
    let files = stub(HashMap::from([
        (
            "src/main.js",
            "export default { boot() { for (;;) {} return 1; } };\n",
        ),
    ]));
    let mut rt =
        PluginRuntime::new_with_instruction_budget(&[], &[], &host_surface(), files, 1_000_000)
            .unwrap();
    rt.load("src/main.js").unwrap();
    match rt.call_default("boot", vec![]) {
        Err(msg) => assert!(
            msg.contains("budget") || msg.contains("budget"),
            "expected instruction budget error, got: {}",
            msg
        ),
        Ok(v) => panic!("infinite loop must not return, got {:?}", v),
    }
}

#[test]
fn test_normal_plugin_within_budget() {
    let files = stub(HashMap::from([
        ("src/main.js", "export default { boot() { return 7; } };\n"),
    ]));
    let mut rt =
        PluginRuntime::new_with_instruction_budget(&[], &[], &host_surface(), files, 1_000_000)
            .unwrap();
    rt.load("src/main.js").unwrap();
    assert_eq!(rt.call_default("boot", vec![]).unwrap().as_f64(), Some(7.0));
}

#[test]
fn test_runtime_export_namespace_level_decision() {
    let available = vec![
        "http.get".to_string(),
        "http.get.api.*".to_string(),
        "storage.<signer>.read".to_string(),
    ];
    let requested = vec![
        "http.get.api.example".to_string(),
        "storage.<signer>.read".to_string(),
    ];
    let names = exported_namespaces(&requested, &available);
    for expected in ["http", "storage", "metado"] {
        assert!(names.contains(&expected.to_string()), "missing {}", expected);
    }
}