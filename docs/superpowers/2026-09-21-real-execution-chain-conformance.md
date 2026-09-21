# 真实执行链路：计划 vs 设计文档 一致性统计

日期：2026-09-21 · 分支 `feat/v1-engine`
设计文档：`docs/superpowers/specs/2026-09-21-real-execution-chain-design.md`
实现计划：`docs/superpowers/plans/2026-09-21-real-execution-chain.md`
用途：执行前对设计文档的**实现 / 跟随 / 超出 / 缩减**情况统计，并附当前实现方案、API 面、预期用户交互场景。

## 1. 统计总览

| 维度 | 数量 | 说明 |
|------|------|------|
| 设计文档节 | 7 | §1–§7 全部有对应实现任务 |
| 实现覆盖（有任务） | 7/7 | 无遗漏节 |
| 逐字/语义跟随项 | 14 | 机制、接口、行为表、验收、测试策略等（见 §2） |
| 超出设计文档的增项 | 2 | `run_mdl_with_storage` 显式目录参数；`DEFAULT_INSTRUCTION_BUDGET` 导出（见 §3） |
| 缩减/偏差项 | 1 | typed `Fault` 改为消息式错误（符合「内置 API 保持基本」，见 §4） |
| 已修正偏差 | 1 | `storage.read` 非 UTF-8 处理已由 `from_utf8_lossy` 改回「→错误」（见 §4） |
| 设计明确非目标（不做） | 6 | http/file/sleep/custom/daemon/异步非阻塞 dispatch |

## 2. 逐节对照（实现 / 跟随）

| 设计节 | 要求 | 计划落点 | 状态 |
|--------|------|----------|------|
| §2 接线机制 | 单一 `__metadoDispatch` 桥；放行方法经桥；host=None 保持 stub；promise 用 `Promise.resolve` 包装且桥不外抛 | Task 2 形状工厂 host 分支 + `dispatch_bridge` | 跟随 |
| §2 | 未放行方法保持 deny 语义 | Task 2 eval 中 `!a` 分支不变 | 跟随 |
| §3 `HostDispatch` | `fn call(&mut self, ns, method, args) -> Result<Value,String>` | Task 1 `host.rs` | 跟随 |
| §3 `new_with_host` | 新增含 `host` 参数；`new`/`new_with_instruction_budget` 委托（host=None） | Task 2 | 跟随 |
| §3 loader | 增 `dispatch` 存在性判定；桥仅 Some 时注册 | Task 2 `has_host` + `host_rc` | 跟随 |
| §4 storage.read | 二次裁决；非 UTF-8 → 错误 | Task 3 `ensure` + `from_utf8`（见 §4 修正） | 跟随 |
| §4 storage.write | 二次裁决；值串化 String/Number/Bool/Null | Task 3 `arg_string` | 跟随 |
| §4 log | `[level] msg`；error→stderr，其余 stdout | Task 3 | 跟随 |
| §4 time.now | UNIX 毫秒 | Task 3 | 跟随 |
| §4 crypto.randomBytes | 真实调用；非法 n → 错误 | Task 3 | 跟随 |
| §4 其余 | `not implemented (v1): ns.method` | Task 3 `_ =>` | 跟随 |
| §5 E2E | 示例 storage-log + `run_real_test` 四项断言 | Task 4 | 跟随 |
| §6 架构锁定 | 内置 API 折叠不变；串化；log 前缀；缺 boot 早返回；storage 目录 env | Task 3/4 | 跟随 |
| §7 测试策略 | 桥 5 测 + RealHost 5 测 + E2E 2 测 + 回归 | Task 2/3/4/5 | 跟随 |

## 3. 超出设计文档的增项（2）

1. **`run_mdl_with_storage(bytes, extra_grant, storage_dir)` 显式目录参数**：设计 §6.5 只定义
   `METADO_STORAGE_DIR` env。计划额外暴露显式参数并由 `run_mdl` 委托（env → 参数），
   目的是让 E2E 测试可注入临时目录、避免 env 并发竞态。**纯增量，不改变用户可见 CLI 行为**。
2. **`DEFAULT_INSTRUCTION_BUDGET` 从 executor re-export**：`run_mdl` 需把预算传入
   `new_with_host`。设计未点名该常量导出，属接线必需的管道改动。

## 4. 缩减 / 偏差

1. **typed `Fault` → 消息式错误**：设计 §4 原述「非 UTF-8 → Fault」等分类。因用户明确
   「内置 API 保持基本功能」，`HostDispatch` 统一 `Result<Value, String>`，错误以消息表达
   （权限拒绝用 `PermissionDenied:` 前缀），不引入 typed 错误枚举。**有意缩减，符合指令。**
2. **`storage.read` 非 UTF-8（已修正）**：计划初稿用 `String::from_utf8_lossy` 静默替换，
   与设计 §4「→错误」不符；已在执行前改为 `String::from_utf8(...).map_err(...)`，现为跟随。
3. **设计非目标，计划同样不做**：http/file/sleep/sha256/hmac/custom 真实化、异步非阻塞
   dispatch、daemon 侧接线、cap 上限/typed 错误。均在设计 §1 明列为非目标。

## 5. 当前实现方案（摘要）

- **桥**：`PluginRuntime::new_with_host` 在 context 建成后，向 `globalThis` 注册唯一
  NativeFunction `__metadoDispatch`（`FunctionObjectBuilder` 构建，闭包捕获
  `Rc<RefCell<Box<dyn HostDispatch>>>`）。形状工厂（eval）放行方法改为调用该桥；
  `is_promise_style` 决定同步返回/抛错或返回已 settle 的 `JsPromise`。
- **分层**：executor 只定义接口，不持有内置实现；cli `RealHost` 注入真实
  storage/log/time/crypto，并在每次真实调用前 `grants_allow` 二次裁决。
- **零回归**：`new`/`new_with_instruction_budget` → `new_with_host(..., None)`，host=None 时
  桥不注册、方法保持 stub，现有测试与 daemon/contract 不受影响。
- **接线**：`run_mdl` 经 `run_mdl_with_storage` 构造 `RealHost` 并传 `new_with_host`。
- **验收**：架构行为（放行/拒绝/真实执行/错误流），不锁定内置 API 返回值编码。

## 6. API 面

新增/变更的 Rust API：

```rust
// executor
pub trait HostDispatch {
    fn call(&mut self, ns: &str, method: &str, args: &[Value]) -> Result<Value, String>;
}
impl PluginRuntime {
    pub fn new_with_host(
        exported: &[String], granted: &[String], surface: &[String],
        files: FilesFn, budget: usize, host: Option<Box<dyn HostDispatch>>,
    ) -> Result<Self, String>;
}
pub const DEFAULT_INSTRUCTION_BUDGET: usize;

// cli
pub struct RealHost { /* storage, granted */ }
impl RealHost {
    pub fn new(storage_dir: PathBuf, granted: Vec<String>) -> Result<Self, String>;
}
impl HostDispatch for RealHost { /* storage/log/time.now/crypto.randomBytes */ }

pub fn run_mdl_with_storage(
    bytes: &[u8], extra_grant: &[String], storage_dir: &Path,
) -> Result<RunOutcome, String>;
// run_mdl(bytes, extra_grant) 签名不变（storage 目录取 METADO_STORAGE_DIR 或 ./metado-storage）
```

插件可见面（不变）：`import { storage, log, time, crypto } from '@metado/runtime'`。
内部新增（插件不可见）：`globalThis.__metadoDispatch(ns, method, ...args)`。

## 7. 预期用户交互场景

1. **基本真实运行**：`mdl run app.mdl` → `boot` 真实读写 storage（默认 `./metado-storage`）、
   `log.info` 打印到 stdout、`time.now`/`crypto.randomBytes` 真实执行；进程内返回真实结果。
2. **自定义存储目录**：`METADO_STORAGE_DIR=/data/metado-store mdl run app.mdl`。
3. **权限拒绝（调用期）**：插件未请求 `storage.write` 却 `await storage.write(...)` →
   异步 reject `permission denied: storage.write`；`mdl run` 以错误退出。
4. **宿主二次裁决拒绝**：形状放行但 `granted` 不含（异常/绕过场景）→ 消息
   `PermissionDenied: storage.write`（纵深防御，正常路径不触发）。
5. **未实现能力**：`http.get(...)`/`file.read(...)`/`time.sleep(...)`/`custom.*` →
   `not implemented (v1): <ns>.<method>`。
6. **非 boot 插件**：manifest 无 `[entries.boot]` → `invoked=false`，不构造 host、无存储副作用。
7. **示例直用**：`examples/storage-log` 可 `mdl build` 后 `mdl run`，观察 storage 回环与日志。

## 8. 结论

计划对设计文档 **7/7 节覆盖、14 项跟随**；超出仅 2 项纯增量（测试可注入目录、常量导出）；
缩减 1 项（typed 错误→消息式）系用户指令下的有意选择；执行前发现的 1 处偏差（UTF-8）已修正。
设计非目标全部不在计划内。**计划可执行。**
