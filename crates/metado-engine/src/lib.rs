//! Metado 核心引擎（v1）：值模型、权限解析、签名验签、容器解析、能力框架、生命周期。
//! 纯 Rust 数据结构与逻辑，无 JS 执行（JS 由 metado-executor / boa 提供）。

pub mod capability;
pub mod container;
pub mod error;
pub mod manifest;
pub mod permission;
pub mod plugin;
pub mod signature;
pub mod trace;
pub mod value;

pub use capability::{CapabilityMeta, CapabilityRegistry, CapabilitySet};
pub use container::Container;
pub use error::{ErrorKind, ExecutionError};
pub use manifest::Manifest;
pub use permission::{
    available_namespaces, exported_namespaces, grants_allow, namespace_of, permission_allows,
    PermissionSet,
};
pub use plugin::{Plugin, PluginState};
pub use signature::{signer_id, KeyPair, SignedBundle};
pub use trace::{TraceEvent, TraceSink};
pub use value::Value;

/// 引擎门面：串起验签 → 容器 → 清单 → 插件生命周期 → 调用（v1 stub 回传输入）。
pub struct Engine {
    capabilities: CapabilityRegistry,
    permission_sets: PermissionSet,
    plugins: Vec<Plugin>,
    trace: TraceSink,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            capabilities: CapabilityRegistry::new(),
            permission_sets: PermissionSet::new(),
            plugins: Vec::new(),
            trace: TraceSink::new(),
        }
    }

    pub fn register_capability(&mut self, cap: Box<dyn CapabilitySet>) {
        self.capabilities.register(cap);
    }

    pub fn define_permission_set(&mut self, name: &str, perms: Vec<String>) {
        self.permission_sets.define(name, perms);
    }

    pub fn load_plugin(
        &mut self,
        bundle: &SignedBundle,
        requested: Vec<String>,
    ) -> Result<(), String> {
        bundle.verify()?;

        let container = Container::from_bytes(bundle.payload())?;
        let toml_bytes = container.read_file("mdl.toml")?;
        let toml_content = std::str::from_utf8(&toml_bytes)
            .map_err(|e| format!("invalid UTF-8 in mdl.toml: {}", e))?;
        let manifest =
            Manifest::from_toml(toml_content).map_err(|e| format!("invalid manifest: {}", e))?;

        // permission-set 引用存在性检查 + 展开（C1；未定义 → 加载失败）。
        let set_expanded = self.permission_sets.expand(&manifest.permission_set)?;
        let mut requested_final = requested;
        for p in set_expanded {
            if !requested_final.iter().any(|r| r == &p) {
                requested_final.push(p);
            }
        }

        let sid = signer_id(&bundle.signer_pubkey());

        // C2 身份闸门：同名加载——同 signer 视为更新（替换旧记录），不同 signer 拒绝（防冒名）。
        if let Some(existing) = self.plugins.iter().find(|p| p.id == manifest.name) {
            if existing.signer_id != sid {
                return Err(format!(
                    "plugin '{}' already loaded by a different signer ({}); refusing impostor",
                    manifest.name, existing.signer_id
                ));
            }
            self.plugins.retain(|p| p.id != manifest.name);
        }

        let mut plugin = Plugin::new(&manifest.name, &sid);
        plugin.manifest = Some(manifest);
        plugin.requested = requested_final.clone();
        plugin.granted = requested_final;
        plugin.transition(PluginState::Installing)?;
        plugin.transition(PluginState::Installed)?;
        plugin.transition(PluginState::Loading)?;
        plugin.transition(PluginState::Pending)?;

        self.trace
            .record(TraceEvent::EntryStart { entry: plugin.id.clone() });
        self.plugins.push(plugin);
        Ok(())
    }

    pub fn plugin(&self, plugin_id: &str) -> Result<&Plugin, String> {
        self.plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| format!("plugin not found: {}", plugin_id))
    }

    /// 越界授予治理（C1）：仅保留 requested 模板覆盖内的权限，去重；返回最新 granted。
    pub fn grant(&mut self, plugin_id: &str, perms: Vec<String>) -> Result<Vec<String>, String> {
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
        let requested = plugin.requested.clone();
        for p in perms {
            let within_requested = requested.iter().any(|r| permission_allows(r, &p));
            if within_requested && !plugin.granted.iter().any(|g| g == &p) {
                plugin.granted.push(p);
            }
        }
        Ok(plugin.granted.clone())
    }

    /// 登记的请求权限全集（含展开的 permission-set；C1 requested 上界）。
    pub fn requested(&self, plugin_id: &str) -> Result<Vec<String>, String> {
        self.plugin(plugin_id)
            .map(|p| p.requested.clone())
            .map_err(|e| e.to_string())
    }

    /// 当前有效授予（engine 权威；越界项永不在此出现）。
    pub fn granted(&self, plugin_id: &str) -> Result<Vec<String>, String> {
        self.plugin(plugin_id)
            .map(|p| p.granted.clone())
            .map_err(|e| e.to_string())
    }

    /// 激活插件：Pending → Active（此后 invoke 可用）
    pub fn activate(&mut self, plugin_id: &str) -> Result<(), String> {
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
        plugin.transition(PluginState::Active)
    }

    pub fn invoke(&self, plugin_id: &str, entry: &str, input: Value) -> Result<Value, ExecutionError> {
        let plugin = self
            .plugins
            .iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| {
                ExecutionError::new(entry, ErrorKind::PluginError, "plugin not found")
            })?;

        if plugin.state != PluginState::Active {
            return Err(ExecutionError::new(
                entry,
                ErrorKind::PluginError,
                "plugin not active",
            ));
        }

        // v1 stub: 返回输入值
        Ok(input)
    }
}