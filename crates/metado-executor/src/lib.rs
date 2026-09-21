//! Metado JS 执行器（v1）：boa 桥接、模块解析、值互转、执行预算、插件运行时。
//! Phase 2 模块将逐一在这里挂载。

pub mod budget;
pub mod engine;
pub mod executor;
pub mod module_loader;
pub mod plugin_runtime;
pub mod virtual_module;

pub use budget::ExecutionBudget;
pub use engine::JsEngine;
pub use executor::Executor;
pub use module_loader::ModuleLoader;
pub use plugin_runtime::{FilesFn, PluginRuntime, TraceHook};
pub use virtual_module::VirtualModule;