//! Metado 核心引擎（v1）：值模型、权限解析、签名验签、容器解析、能力框架、生命周期。
//! 纯 Rust 数据结构与逻辑，无 JS 执行（JS 由 metado-executor / boa 提供）。

pub mod error;
pub mod manifest;
pub mod signature;
pub mod value;