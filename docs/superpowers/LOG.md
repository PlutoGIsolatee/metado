# Metado 实施日志

分支 `feat/v1-engine` · 严格 TDD（RED → GREEN → 每任务提交）· 计划仅供参考

---

## 2026-09-20（会话续） Phase 4 收尾 + Phase 5 引擎进程/IPC

### 今日完成

**Phase 4 收尾**
- **Task 4.6 `mdl env`（提交 1943812）**：能力/权限静态诊断。
  - `EnvReport{plugin_name, signer, permission_sets, requested, available, exported_namespaces, entries, ungranted_requests}`。
  - 导出语义落定：`exported = requested ∩ available`（未请求的能力导出不存在，恒含 `metado`）。
  - 人类输出表 + `--json`；缺宿主能力（如 `custom.alert`）单独警示。
  - 同时完成 PluginRuntime 契约重构：`new(available, granted, files)` → `new(exported: &[String], files)`，导出存在性全部由宿主裁定（§4.3）。
  - 真机 E2E：`storage.read+log.info+custom.alert` 插件 → export 仅 `log, metado, storage`，`custom.alert` 被标 missing host capability。正确。

- **Task 4.7 `mdl trace`（提交 d1f75ec）**：执行轨迹观测（§12.6）。
  - engine `TraceEvent` 加 `Serialize`。
  - `PluginRuntime::set_trace`：入口起止（EntryStart/End）+ 值流转（ValueFlow in/out）在 `call_default` 时发射；每个导出的能力命名空间在 runtime 合成模块初始化时发射一条 CapabilityCall（v1 记"能力路由描述 + 放行裁决"，stub 调用体 Phase 5 才产真实调用——故改写为初始化时发射，绕开 0.22 NativeFunction→JsValue 的构造限制）。
  - `mdl trace <file> [--filter kind] [--json]`：人类树 / 类型过滤 / JSON 数组。
  - 排错：`events` 移入 sink 闭包后读取需 `Rc<RefCell>`；`Vec<TraceEvent>` `into_inner` 语义错误改 `borrow().clone()`；Rc 赋值需 RefCell。

**Phase 5 引擎进程 + IPC（metado-ipc × metado-daemon）**
- **Task 5.1 传输层（d7001d7）**：`Transport{send,receive,close}` 消息级语义；帧协议 = 4 字节大端长度前缀 + 载荷，上限 16 MiB（防放大）；UnixListener/UnixTransport + `unix_server` accept 循环。4 测试（往返/多帧/空+512KiB/关后发送失败）。
- **Task 5.2 JSON-RPC 2.0（8b76918）**：Request（含通知）、Response（result×or error）、ErrorObject 标准错误码（-32700 解析、-32600 非法请求、-32601 方法未找到、-32602 参数错误、-32603 内部错误）；builder 与 getter 同名冲突（`method`/`id`/`params`）→ 改 `named()`/`with_id()`/`with_params()`。7 测试。
- **Task 5.3 协议定义（dc1bc68）**：12 个管理方法 + 2 个回调面（dispatch/notify）常量；`Params` 枚举（LoadPlugin/Grant/Revoke/SetLifecycle/Invoke/ListPlugins/Uninstall/Purge/RegisterPermissionSet/SetDomainConfig/SetActive/Trace）带 `from_method` 载荷校验（缺字段→BadParams、未知方法→-32601）；`handle_one` 分发：请求→响应（错误映射 ErrorObject），通知→抑制回复。9 测试。
- **Task 5.4/5.5 Android/Windows 传输 → 计划回写延后**：宿主（Termux/Linux，target_os=linux）无法编译/测试目标机 IPC，按"质量与正确性优先"原则不写无法验证的代码；Transport trait 与帧协议为其预留载体。
- **Task 5.6 Daemon（40584e6）**：
  - `Daemon` 库：engine 生命周期（load_plugin/grant/define_permission_set）+ 可执行记录表（manifest/container/exported/granted/active）；12 方法分发；invoke/trace 走 PluginRuntime 真实 ESM；错误转标准 JSON-RPC 错误对象。
  - `serve_unix`：bind → accept 循环 → 逐连接逐帧 `handle_rpc` → 对端关帧即关连接。
  - `metado-daemon --socket PATH` 二进制。
  - 7 库测试含 UDS 全栈 roundtrip（loadPlugin → invoke 跨 Unix socket）。E2E 真机：`loadPlugin` 装载 hex .mdl → `listPlugins` → `invoke envdemo:boot` 返回 `"ok"` → `trace` 出 3 条 CapabilityCall + 入口生命周期 → 未知方法 `-32601` → `purge`。

### 排错记录（本次）
1. `available` move 后又借用 → 先算 exported 再构结构体。
2. Rc 字段赋值 `self.loader.trace = Some(rc)` → `*self.loader.trace.borrow_mut()`（字段改 `RefCell<Option<…>>`）。
3. `?` 对 ProtocolError/IpcError 混用不转换 → 通知分支直接吞错误调用。
4. `.map().collect()` 需显式 `::<Vec<String>>`（Termux 旧 rustc 类型推断弱，7 处）。
5. `Ref` 临时借用跨返回值 → `let evs = borrow().clone()`。
6. 测试先写错协议方法（`domainConfig` getter 不存在）→ 回写 `setDomainConfig` 仅登记断言。
7. `events` RefCell 移入 sink 闭包 → 共享 `Rc<RefCell<_>>`。
8. 一次性串行化 id 类型（jsonrpc）：`&Value` 不实现 `From<u64>` → 断言用 `as_u64()`/`json!`.

### 当前状态
- **workspace 回归：161 passed，0 failed，0 warning。**
- 提交：1943812, d1f75ec, d7001d7, 8b76918, dc1bc68, 40584e6。
- Phase 1-5 全部完成。计划 Task 打勾：3.x、4.1-4.7、5.1-5.3、5.6（5.4/5.5 延迟+N论据）。

### 下一步
- Task 6.1/6.2：examples/hello-plugin、examples/http-plugin（Phase 6 契约测试）。
- Task 6.3+：`mdl run`/`mdl test` 对接 daemon（IPC 客户端面）；日志系统接入 daemon 输出。

---

## 2026-09-20（会话三段） Phase 6 Examples + Contract Suite

### 今日完成

**`mdl run` 真实 ESM（10d8cd5）**
- run.rs 由"外壳 + engine::invoke 占位"强化：engine 生命周期门控（load/activate）+ PluginRuntime 真实执行 boot ESM；`RunOutcome.result` 现在是真的 JS 输出（`"hello from metado"`）。
- 新增 run_real_esm 测试；run_test 断言改为 `Value::String("hello")`。

**Task 6.1-6.3 Examples（d3552ca）**
- **hello-plugin**：boot+onMessage 双入口；`mdl run`/`mdl test` 通过。
- **http-plugin**：manifest 不声明 http —— 无 `--grant` `mdl run` 链接失败（导出发析，rc=1，正确）；`--grant=http.get` → `typeof http === "function"`、`mdl test` 为 true。**关键修正**：cap-http 权限是 `http.get/http.post/http.get.api.*`，`http.fetch` 不是合法权限（初稿用错导致带 grant 也链接失败）。
- **custom-cap-plugin**：v1 **无 `#[capability]` proc-macro** → 扩展面 = `CapabilitySet` trait（`meta() -> CapabilityMeta{name,permissions,exports}`）注册进 `CapabilityRegistry`。CLI/daemon 内置宿主仅注册六内置 → `custom.alert` 如实报 missing host capability（正确行为）。README 含扩展代码示例。
- 计划 Task 6.3 回写：去掉 macro 措辞，改为 CapabilitySet trait 扩展链路。

**Task 6.4 Contract Test Suite（tests/contract，workspace `metado-contract`）**
- 跨 crate 黑盒对齐（CLI ⇄ engine ⇄ executor ⇄ daemon），24 测试全绿：
  - **权限模型契约**（§4.3）：exported = requested ∩ available ∪ {metado}；未授权命名空间 import 链接失败；额外 grant 不越 available 界；env 忠实列出 ungranted_requests；run 与 test 对同一授权导出一致。
  - **生命周期状态机契约**：正常 install 链、Evicted 可重返 Active、任何状态可终态 Uninstalled、非法迁移（Active→Installing、终态→Active）拒绝且状态不变、篡改 bundle load 被拒。
  - **签名契约**：任意比特翻转被拒（magic 破坏走 from_bytes、载荷破坏走 verify）；换签名保持 payload、signer_id 稳定复算；错钥 verify 拒绝。
  - **容器契约**：roundtrip 保文件、`../` 穿越条目入库即拒、缺 mdl.toml 拒。
  - **CLI⇄daemon 对齐**：daemon invoke 与 mdl run 结果一致、篡改 load 双端拒绝、revoke 后 invoke 等同未授权、listPlugins 条目稳定排序、test 语义（entries.test 优先于 boot 回退）、daemon trace 输出补 `kind` 字段与 CLI 对齐。
  - **宿主自定义能力注册链路**：注册 AlertCapability → custom.alert 进 available → exported 含 custom → import 链接 → boot 返回 typeof。
- 顺带修复 daemon：listPlugins 按 entries 名排序（HashMap 无序破坏契约测试）；trace JSON 事件加 `kind`。engine：`Container::from_bytes` 入库时拒绝 `..`/绝对路径条目（纵深防御，read_file 早已挡）。

### 排错记录（本次）
1. `tests/contract/Cargo.toml` 相对路径错（`../crates/` → `../../crates/`），lib 的 `pub mod common` 无法被 `tests/` 集成测试引用 → 改用 `use metado_contract::common::…`。
2. 契约测试 helper 与局部变量同名（`exported` fn vs Vec 绑定）→ E0308“expected function”；改名 `exported_list`。
3. daemon listPlugins 返回 HashMap 无序 entries → 排序。
4. daemon trace 事件无 `kind`（序列化是 `{"EntryStart":{…}}`）→ TRACE 分发时补 `kind`。
5. 自建 `CapabilityRegistry` 只注册 custom 缺六个内置 → available 缺 log.info；改用 `metado_cli::registry_permissions()` ∪ custom。
6. 位翻转测试盲目 `from_bytes().unwrap()`：翻转 magic 字节 → from_bytes Err；改为"from_bytes Err 或 verify Err 均视为被拒"。
7. `Container` 未拒 `..` 入库条目 → 补 from_bytes 校验。

### 当前状态
- **contract 24/24 全绿。**
- 提交：10d8cd5, d3552ca（examples）；engine/daemon 改动待回归后与 contract 一并提交。
- 计划勾选：4.6/4.7、5.1-5.3/5.6、6.1-6.4。

### 下一步
- `cargo test --workspace` 全量回归（含受影响 engine/daemon/CLI）。
- Task 6.x+：Node 侧 dev shim（检查 node 可用性）；后续 Phase 7+。