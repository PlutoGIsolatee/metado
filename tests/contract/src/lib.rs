//! Metado 契约测试套件（Task 6.4）：跨 crate 黑盒对齐。
//!
//! 覆盖：
//! - 权限模型（requested/granted/available → exported，§4.3）
//! - 生命周期状态机（§6，含非法迁移拒绝与终态）
//! - 签名校验（篡改任何比特失败 / 换签保持 payload / 错钥拒绝）
//! - 容器格式（roundtrip / `../` 穿越拒绝 / 缺 mdl.toml）
//! - CLI ⇄ daemon 生产行为对齐（invoke/list/revoke/test/trace 语义一致）
//! - 宿主自定义能力注册链路（CapabilitySet trait 即 v1 扩展点）

pub mod common;