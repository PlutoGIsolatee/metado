# 处置台账核验评审（2026-09-21）

日期：2026-09-21 · 分支 `feat/v1-engine` · 来源：`docs/superpowers/2026-09-21-project-review-disposition.md`

> 核验方式：对台账声明的修复提交（`336c8b9`、`531112c` 等）逐个**对照实际代码**核实，并复跑全量回归。
> 基线实测：`cargo test --workspace --jobs 1` → 61 个测试二进制，218 passed，0 failed，0 warning。

## 1. 总体判定

台账**基本准确、诚实**——每项声明均可依据源码核实，未虚报完成。剩余 ⬜/🟡 项与代码现状一致（如 daemon REVOKE 仍是 `records.remove`、JSON-RPC 语法错误静默 drop、lifecycle 仍为摆设），台账如实登记为待办。

## 2. 已核实为真（关键项对码结论）

| 项 | 证据 |
|----|------|
| C1 `granted ⊆ requested` 引擎唯一权威 | `Engine::grant` 按 requested 模板段级过滤+去重（`crates/metado-engine/src/lib.rs:112-126`）；`load_plugin` 做 permission-set 存在性检查+展开（`:67-74`）；daemon GRANT 过引擎裁决后落记录（`crates/metado-daemon/src/lib.rs:201-216`）；CLI run 从 `engine.requested()/granted()` 取执行面（`crates/metado-cli/src/run.rs:78-88`） |
| C2 同名 signer 身份闸门 | 同名加载：同 signer 替换、异 signer 拒绝且原插件保留（`lib.rs:79-87`） |
| C3 指令燃料（部分） | `instructions_remaining(budget)` 生效（`crates/metado-executor/src/plugin_runtime.rs:277`，默认 50M）；紧循环终结测试存在且通过（`tests/plugin_runtime_test.rs:350-355`）；"never-settling await 仍挂起"限制与代码注释一致（`:382`）——诚实 |
| C4 运行时放行 + trace 修正 | `PluginRuntime::new(exported, granted, surface, files)`（`:248`）；异步 reject / 同步 throw（`:108-112`）；CapabilityCall 传真实 granted + 发射 PermissionCheck（`:130-143`） |
| C5 导出形状统一 | 命名空间对象 + 方法函数（`:102-145`）；示例断言改 `typeof http === "object"` |

## 3. 需修正/补充（4 处，均小）

1. **测试二进制数不符**：「64 测试二进制」与实测 61 不符（实质声明 0 failed/0 warning 成立），数字建议更正。
2. **C5 有未覆盖的方法面缺口**：`build_ns_methods` 取 `permission.split('.')[1]` 为方法名（`plugin_runtime.rs:46-54`）。对 signer 形式 `storage.<signer>.read` → 方法名变为字面 `<signer>`，storage 命名空间会多出假方法。当前因 cap-storage 同时注册裸 `storage.read/write`（`crates/metado-cap-storage/src/lib.rs:17-20`）而被掩盖；run_test 的 `<signer>` 用例（`crates/metado-cli/tests/run_test.rs:36-56`）只断言 `typeof`，未验证方法面。建议补「仅请求 `<signer>` 形式时 `storage.read/write` 仍为方法」用例，或先做 `<signer>` 绑定再提取方法。
3. **旧 `PermissionResolver` 仍为死代码**：`permission.rs:4-78` 的 exact-match 版 `matches`/`can_grant` 全 crate 无调用，却仍被 re-export（`lib.rs:20`），与新 `permission_allows` 并存。C1 收敛后建议删除以免语义混淆。
4. **trace 字段仍近似**：CapabilityCall 的 `requested` 仍填 `export_names`（导出的命名空间名，`plugin_runtime.rs:136`）而非真实 requested 权限集。C4 已修"谎报 granted"，requested 字段仍非精确值，建议改为真实 requested 或改名。

## 4. 建议

台账可信度高，是真实的处置记录而非目标声明。建议：
- 补 1 条 `<signer>` 形式的方法面测试（见 3.2）——这同时覆盖 C5 与 signer 绑定链路的真实面。
- 删除 `PermissionResolver` 死代码（见 3.3）。
- 更正测试二进制计数、修正 trace requested 字段（见 3.1、3.4）。

以此台账作为持续闭环基线，后续评审聚焦其 ⬜ 簇（容量上限、IPC 错误路径、信封字段、生命周期语义）。