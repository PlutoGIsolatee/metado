# Custom Capability Plugin (Task 6.3)

宿主自定义能力演示。**v1 无 `#[capability]` proc-macro**（计划回写）：
扩展面是 `metado_engine::CapabilitySet` trait——实现 permission 全集声明，
注册进 `CapabilityRegistry` 即成为 `available` 的一部分，`custom` 命名空间
随之可导出（requested ∩ available，§4.3）。

内置 CLI/daemon 宿主只注册六内置能力，因此对 **本示例** 会如实报告
`custom.alert` 为 missing host capability（这正是未注册能力的正确诊断）：

```console
mdl build examples/custom-cap-plugin --key /tmp/custom.key --output /tmp/custom.mdl
mdl env /tmp/custom.mdl    # exported=metado; "!!! missing host capability: custom.alert"
mdl test /tmp/custom.mdl   # import { custom } 链接失败（export 不存在）
```

宿主注册该能力后的完整链路（load → granted 含 custom.alert → exported 含 custom →
import 链接成功 → `custom` 类型为 function）由契约测试套件覆盖
（`tests/contract` 的 `test_custom_capability_host_registration`），
并按 `CapabilitySet` trait 演示能力分派契约。

```rust
// 自定义能力声明（宿主侧）——CapabilitySet 即 v1 扩展点
struct AlertCapability;
impl metado_engine::CapabilitySet for AlertCapability {
    fn meta(&self) -> metado_engine::CapabilityMeta {
        CapabilityMeta {
            name: "custom".into(),
            permissions: vec!["custom.alert".into()],
            exports: vec!["alert".into()],
        }
    }
}
engine.register_capability(Box::new(AlertCapability));
```