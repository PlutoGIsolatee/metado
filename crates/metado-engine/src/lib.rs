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
pub use permission::{PermissionResolver, PermissionSet};
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
        granted: Vec<String>,
    ) -> Result<(), String> {
        bundle.verify()?;

        let container = Container::from_bytes(bundle.payload())?;
        let toml_bytes = container.read_file("mdl.toml")?;
        let toml_content = std::str::from_utf8(&toml_bytes)
            .map_err(|e| format!("invalid UTF-8 in mdl.toml: {}", e))?;
        let manifest =
            Manifest::from_toml(toml_content).map_err(|e| format!("invalid manifest: {}", e))?;

        let sid = signer_id(&bundle.signer_pubkey());

        let mut plugin = Plugin::new(&manifest.name, &sid);
        plugin.manifest = Some(manifest);
        plugin.granted = granted;
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

    pub fn grant(&mut self, plugin_id: &str, perms: Vec<String>) -> Result<(), String> {
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
        plugin.granted.extend(perms);
        Ok(())
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