//! `mdl test`（Task 4.5）：真实 ESM 执行下的插件契约测试。
//! - 用例入口：`[entries.test] export = "test"`；未声明则回退 `boot` 冒烟
//! - JS 用例返回真值 = PASS，假/抛错 = FAIL（harness 不整体失败，逐用例报告）
//! - 未声明任何入口 = 单条 FAIL 说明，不阻塞（反馈循环友好）

use std::collections::HashMap;

use metado_cap_crypto::MetaCrypto;
use metado_cap_file::MetaFile;
use metado_cap_http::MetaHttp;
use metado_cap_log::MetaLog;
use metado_cap_storage::MetaStorage;
use metado_cap_time::MetaTime;
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

/// 收集内置能力申明的 permission 全集（供 available 判定）。
fn available_permissions() -> Vec<String> {
    let mut reg = metado_engine::CapabilityRegistry::new();
    for cap in [
        Box::new(MetaLog) as Box<dyn metado_engine::CapabilitySet>,
        Box::new(MetaTime),
        Box::new(MetaCrypto),
        Box::new(MetaStorage),
        Box::new(MetaFile),
        Box::new(MetaHttp),
    ] {
        reg.register(cap);
    }
    reg.all_permissions()
}

pub fn test_mdl(bytes: &[u8], extra_grant: &[String]) -> Result<Vec<TestOutcome>, String> {
    let bundle = SignedBundle::from_bytes(bytes)?;
    bundle.verify()?;

    let container = Container::from_bytes(bundle.payload())?;
    let manifest_raw = container.read_file("mdl.toml")?;
    let manifest_raw = String::from_utf8(manifest_raw).map_err(|e| format!("manifest utf8: {}", e))?;
    let manifest = Manifest::from_toml(&manifest_raw).map_err(|e| format!("manifest: {}", e))?;

    let available = available_permissions();
    let mut granted = manifest.permission.clone();
    granted.extend(extra_grant.iter().cloned());

    // 容器文件树 → 运行时 FilesFn
    let files: HashMap<String, Vec<u8>> = container
        .files()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let files = Box::new(move |rel: &str| files.get(rel).cloned());

    let mut rt = PluginRuntime::new(&available, &granted, files)?;
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