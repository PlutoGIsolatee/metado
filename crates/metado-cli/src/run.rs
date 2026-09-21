//! `mdl run` 主机接线（Task 4.3 → Phase 6 强化）：验签 → 注册内置能力 → engine 生命周期
//! （load → activate）→ `--grant` 并集 → PluginRuntime 真实 ESM 执行 boot 入口。
//! 授权策略（v1）：granted = manifest.permission ∪ `--grant`；导出命名空间 = requested 命名空间 ∩
//! available 命名空间 ∪ {metado}（命名空间级静态面，§4.3）；调用放行 = 方法级动态面（granted 裁决）。

use std::collections::HashMap;

use crate::env::{exported_namespaces, registry_permissions};
use metado_cap_crypto::MetaCrypto;
use metado_cap_file::MetaFile;
use metado_cap_http::MetaHttp;
use metado_cap_log::MetaLog;
use metado_cap_storage::MetaStorage;
use metado_cap_time::MetaTime;
use metado_engine::{signer_id, Container, Engine, Manifest, SignedBundle, Value};
use metado_executor::PluginRuntime;

#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    pub plugin_id: String,
    pub signer: String,
    pub entry: String,
    pub invoked: bool,
    pub result: Value,
}

/// 内置能力全集（v1 host）——注册后 `available` 完整。
pub fn register_builtins(engine: &mut Engine) {
    for cap in [
        Box::new(MetaLog) as Box<dyn metado_engine::CapabilitySet>,
        Box::new(MetaTime),
        Box::new(MetaCrypto),
        Box::new(MetaStorage),
        Box::new(MetaFile),
        Box::new(MetaHttp),
    ] {
        engine.register_capability(cap);
    }
}

pub fn run_mdl(bytes: &[u8], extra_grant: &[String]) -> Result<RunOutcome, String> {
    let bundle = SignedBundle::from_bytes(bytes)?;
    bundle.verify()?;

    let container = Container::from_bytes(bundle.payload())?;
    let manifest_raw = container.read_file("mdl.toml")?;
    let manifest_raw = String::from_utf8(manifest_raw).map_err(|e| format!("manifest utf8: {}", e))?;
    let manifest = Manifest::from_toml(&manifest_raw).map_err(|e| format!("manifest: {}", e))?;

    let mut engine = Engine::new();
    register_builtins(&mut engine);

    // requested（含 permission-set 展开，engine 权威）→ 加载即授予（C1：granted ⊆ requested）
    let mut requested = manifest.permission.clone();
    requested.extend(extra_grant.iter().cloned());
    engine.load_plugin(&bundle, requested)?;
    let plugin_id = manifest.name.clone();
    engine.activate(&plugin_id)?;

    let signer = signer_id(&bundle.signer_pubkey());
    let boot_export = manifest.entries.get("boot").map(|e| e.export.clone());

    let boot_export = match boot_export {
        Some(entry) => entry,
        None => {
            return Ok(RunOutcome {
                plugin_id,
                signer,
                entry: String::new(),
                invoked: false,
                result: Value::Null,
            });
        }
    };

    // 执行面：真实 ESM 容器（与 mdl test/trace/daemon 一致）
    let available = registry_permissions();
    let requested = engine.requested(&plugin_id)?;
    let exported = exported_namespaces(&requested, &available);
    let granted = engine.granted(&plugin_id)?;

    let files: HashMap<String, Vec<u8>> = container
        .files()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let files = Box::new(move |rel: &str| files.get(rel).cloned());

    let mut rt = PluginRuntime::new(&exported, &granted, &available, files)?;
    rt.load("src/main.js")
        .map_err(|e| format!("load entry src/main.js: {}", e))?;
    let result = rt
        .call_default(&boot_export, vec![])
        .map_err(|e| format!("call {}: {}", boot_export, e))?;

    Ok(RunOutcome {
        plugin_id,
        signer,
        entry: boot_export,
        invoked: true,
        result,
    })
}