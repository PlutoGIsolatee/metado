//! `mdl test`（Task 4.5）：真实 ESM 执行下的插件契约测试。
//! - 用例入口：`[entries.test] export = "test"`；未声明则回退 `boot` 冒烟
//! - JS 用例返回真值 = PASS，假/抛错 = FAIL（harness 不整体失败，逐用例报告）
//! - 未声明任何入口 = 单条 FAIL 说明，不阻塞（反馈循环友好）

use std::collections::HashMap;

use crate::env::{exported_namespaces, registry_permissions, resolve_requested};
use metado_engine::{Container, Manifest, SignedBundle};
use metado_executor::PluginRuntime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutcome {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

fn truthy(value: &metado_engine::Value) -> bool {
    match value {
        metado_engine::Value::Null => false,
        metado_engine::Value::Bool(b) => *b,
        metado_engine::Value::Number(n) => *n != 0.0,
        _ => true,
    }
}

pub fn test_mdl(bytes: &[u8], extra_grant: &[String]) -> Result<Vec<TestOutcome>, String> {
    let bundle = SignedBundle::from_bytes(bytes)?;
    bundle.verify()?;

    let container = Container::from_bytes(bundle.payload())?;
    let manifest_raw = container.read_file("mdl.toml")?;
    let manifest_raw = String::from_utf8(manifest_raw).map_err(|e| format!("manifest utf8: {}", e))?;
    let manifest = Manifest::from_toml(&manifest_raw).map_err(|e| format!("manifest: {}", e))?;

    let available = registry_permissions();
    // requested = 声明 ∪ permission-set 展开 ∪ extra（C1）；导出与授予都以 real requested 为准
    let requested = resolve_requested(&manifest, extra_grant)?;
    let exported = exported_namespaces(&requested, &available);
    let granted = requested.clone();

    // 容器文件树 → 运行时 FilesFn
    let files: HashMap<String, Vec<u8>> = container
        .files()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let files = Box::new(move |rel: &str| files.get(rel).cloned());

    let mut rt = PluginRuntime::new(&exported, &granted, &available, files)?;
    let entry_module = "src/main.js";
    rt.load(entry_module)
        .map_err(|e| format!("load entry {}: {}", entry_module, e))?;

    // 用例选择：entries.test → test；否则 entries.boot → boot 冒烟；都无 → 单条 FAIL 说明
    let case = manifest
        .entries
        .get("test")
        .map(|e| ("test".to_string(), e.export.clone()))
        .or_else(|| {
            manifest
                .entries
                .get("boot")
                .map(|e| ("boot".to_string(), e.export.clone()))
        });

    let outcome = match case {
        Some((name, method)) => {
            match rt.call_default(&method, vec![]) {
                Ok(value) => {
                    let passed = truthy(&value);
                    let detail = if passed {
                        format!("value {}", value.to_json_string().unwrap_or_else(|_| "<value>".into()))
                    } else {
                        "falsy result".into()
                    };
                    TestOutcome { name, passed, detail }
                }
                Err(msg) => TestOutcome {
                    name,
                    passed: false,
                    detail: msg,
                },
            }
        }
        None => TestOutcome {
            name: "no-entry".into(),
            passed: false,
            detail: "manifest declares no [entries.test] nor [entries.boot]".into(),
        },
    };
    Ok(vec![outcome])
}