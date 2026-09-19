//! Metado JS 执行器（v1）：boa 桥接、模块解析、值互转、执行预算。
//! Phase 2 模块将逐一在这里挂载。

pub mod engine;

pub use engine::JsEngine;