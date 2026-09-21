//! `mdl env`（Task 4.6）：能力/权限静态诊断（§12.5）。
//! 输出 requested（manifest 声明）、available（能力注册表）、
//! 实际导出命名空间（§4.3：未请求的能力导出不存在 → 导出 = requested ∩ available）、
//! 入口表、权限集名、以及请求了但注册表缺失的权限。

use metado_cap_crypto::MetaCrypto;
use metado_cap_file::MetaFile;
use metado_cap_http::MetaHttp;
use metado_cap_log::MetaLog;
use metado_cap_storage::MetaStorage;
use metado_cap_time::MetaTime;
use metado_engine::{
    available_namespaces, namespace_of, CapabilityRegistry, Container, Manifest, SignedBundle,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvReport {
    pub plugin_name: String,
    pub signer: String,
    pub permission_sets: Vec<String>,
    pub requested: Vec<String>,
    pub available: Vec<String>,
    /// 实际导出命名空间（§4.3：未请求 → 不存在），恒含 `metado`
    pub exported_namespaces: Vec<String>,
    pub entries: Vec<(String, String)>,
    /// 请求了但能力注册表缺失（宿主需要另行实现）
    pub ungranted_requests: Vec<String>,
}

/// 内置能力注册表（builtin host）申明的 permission 全集。
pub fn registry_permissions() -> Vec<String> {
    let mut reg = CapabilityRegistry::new();
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

/// 实际导出命名空间 = 命名空间 ∈ available 命名空间的请求命名空间 ∪ {metado}
/// （§4.3：未被支持的命名空间 → 导出不存在；粒度是命名空间而非权限字面量）
pub fn exported_namespaces(requested: &[String], available: &[String]) -> Vec<String> {
    metado_engine::exported_namespaces(requested, available)
}

/// 请求了但能力注册表缺失（宿主需要另行实现）：命名空间不在 available 命名空间集内。
pub fn ungranted_requests(requested: &[String], available: &[String]) -> Vec<String> {
    let avail = available_namespaces(available);
    requested
        .iter()
        .filter(|p| !avail.iter().any(|ns| ns == namespace_of(p)))
        .cloned()
        .collect()
}

pub fn env_mdl(bytes: &[u8]) -> Result<EnvReport, String> {
    let bundle = SignedBundle::from_bytes(bytes)?;
    bundle.verify()?;

    let container = Container::from_bytes(bundle.payload())?;
    let manifest_raw = container.read_file("mdl.toml")?;
    let manifest_raw = String::from_utf8(manifest_raw).map_err(|e| format!("manifest utf8: {}", e))?;
    let manifest = Manifest::from_toml(&manifest_raw).map_err(|e| format!("manifest: {}", e))?;

    let available = registry_permissions();
    let requested = manifest.permission.clone();
    let ungranted_requests = ungranted_requests(&requested, &available);

    let mut entries: Vec<(String, String)> = manifest
        .entries
        .iter()
        .map(|(name, def)| (name.clone(), def.export.clone()))
        .collect();
    entries.sort();

    let exported = exported_namespaces(&requested, &available);

    Ok(EnvReport {
        plugin_name: manifest.name,
        signer: metado_engine::signer_id(&bundle.signer_pubkey()),
        permission_sets: manifest.permission_set,
        requested: manifest.permission,
        available,
        exported_namespaces: exported,
        entries,
        ungranted_requests,
    })
}