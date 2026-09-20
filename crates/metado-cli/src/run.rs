//! `mdl run` 主机接线（Task 4.3）：验签 → 注册内置能力 → load → grant → activate → invoke boot 入口。
//! 授权策略（v1）：manifest 声明的 permission ∪ `--grant` 模拟授权；permission-set 名展开由权限解析器承接。

use metado_cap_crypto::MetaCrypto;
use metado_cap_file::MetaFile;
use metado_cap_http::MetaHttp;
use metado_cap_log::MetaLog;
use metado_cap_storage::MetaStorage;
use metado_cap_time::MetaTime;
use metado_engine::{signer_id, Container, Engine, Manifest, SignedBundle, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct RunOutcome {
    pub plugin_id: String,
    pub signer: String,
    pub entry: String,
    pub invoked: bool,
    pub result: Value,
}

/// 内置能力全集（v1 host）——注册后 `available` 完整、`granted ⊆ available` 才放行。
fn register_builtins(engine: &mut Engine) {
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

    // granted = 插件声明 ∪ CLI 模拟授权
    let mut granted = manifest.permission.clone();
    granted.extend(extra_grant.iter().cloned());
    granted.extend(
        manifest
            .permission_set
            .iter()
            .flat_map(|set| resolve_path_dots(set)),
    );

    engine.load_plugin(&bundle, granted)?;
    let plugin_id = manifest.name.clone();
    engine.activate(&plugin_id)?;

    let boot_export = manifest
        .entries
        .get("boot")
        .map(|e| e.export.clone());

    match boot_export {
        Some(entry) => {
            let result = engine
                .invoke(&plugin_id, &entry, Value::Null)
                .map_err(|e| e.message)?;
            Ok(RunOutcome {
                plugin_id,
                signer: signer_id(&bundle.signer_pubkey()),
                entry,
                invoked: true,
                result,
            })
        }
        None => Ok(RunOutcome {
            plugin_id,
            signer: signer_id(&bundle.signer_pubkey()),
            entry: String::new(),
            invoked: false,
            result: Value::Null,
        }),
    }
}

/// 权限集名展开辅助：`storage` → `storage.<signer>.read/write` 等价族（v1 保守展开，仅保留原 token 校验）。
fn resolve_path_dots(set: &str) -> Vec<String> {
    // permission-set 名由宿主 define_permission_set 预定义；
    // v1 CLI 无宿主策略文件，声明即保留（权限解析器校验 granted ⊆ available 兼容）。
    vec![set.to_string()]
}