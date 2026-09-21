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
- Task N.x Node dev shim 待做（node 可用性已确认）。

---

## 2026-09-20（会话四段） Task 6.4 全绿 + @metado/runtime dev shim

### 今日完成

**Task 6.4 Contract Test Suite（1404d68）**
- 见上段。24/24 全绿；workspace 全量回归 **63 测试二进制全 ok、0 failed、0 warning**。
- 发现并修复两处产品缺口（由契约测试逼出）：
  - `Container::from_bytes` 入库时即拒 `..`/绝对路径条目（旧实现仅 read_file 挡）。
  - daemon `listPlugins` entries 排序稳定；`trace` 事件补 `kind`（与 CLI to_json 对齐）。
- 旧 engine container_test 断言更新（`..` 入库即拒，非读时才拒）。

**Task N.1-N.7 Node dev shim（`packages/runtime-node`，@metado/runtime@0.1.0-dev，零新依赖）**
- 回写：计划原稿 TS(.ts+tsconfig)，TS 属 §15"v1 明确不实现"，且 tsc 为新依赖 → 纯 ESM `.mjs` + JSDoc；不发布外网注册表（仅本地 `npm pack --dry-run` 校验）。
- `errors.mjs`（MetadoError/ExecutionError/PermissionDeniedError）、`stub.mjs`（授权 → 未授权抛 PermissionDenied，已授权抛 "not available in Node"）、`grant.mjs`（Set + grant/grantAll/reset/isGranted）、`test-utils.mjs`（`__METADO_TEST__`）。
- 能力模块权限名与真实 metado-cap-* 注册集逐一核对：http(get/post/get.api.*)、storage(read/write)、file(read/readText)、time(now/sleep)、log(info/warn/error/debug)、crypto(randomBytes/sha256/hmac)（初稿 hash/random/sign/verify 已按真实集更正）。
- `custom.mjs`：Proxy 惰性返回函数（`custom.alert.call()`），访问不再同步抛（assert.rejects 捕获不到同步 getter 抛）。
- `shim.mjs`：Buffer/path/EventEmitter 再导出。
- `tests/permission.test.mjs`：11 断言（10 能力用例 + shim），package self-reference 导入 `@metado/runtime[/test]`，node:assert 零测试框架依赖。`npm test` 全过。

### 排错记录（本次）
1. pathspec `LOG.md` 大小写未命中 → 正确路径。
2. assert.rejects 无法捕获 Proxy get 同步抛 → get 返回异步函数（惰性）。
3. shim 权限名虚构（storage.delete/file.remove/crypto.hash）→ 对照注册集更正。
4. 位翻转 magic 字节会 `from_bytes` Err → 契约测试"from_bytes Err 或 verify Err 均视为被拒"。
5. `exported` 局部变量遮蔽同名函数 → E0308。

### 当前状态
- **Node v26.4.0 + npm 11 可用**；shim 全测试绿。
- 提交：1404d68（contract + hardening）；本次将提交 packages/runtime-node + 计划/LOG 回写。
- 计划勾选：N.1-N.7。

### 下一步
- 提交 Node shim。
- Node 侧 dev shim 后续：插件桌面开发时导入对齐（Phase 7+ 前置）；日志系统接入 daemon 输出。
---

## 2026-09-21（会话五段） 评审修复：C5 导出形状 + C4 运行时放行（一个 TDD 单元）

> 依据 `docs/superpowers/2026-09-20-project-review.md` Critical 5 + Critical 4 + 关联 Important
> （async 入口、glob 权限），用户已确认 4 个微决策后开工。以锁定 spec 为唯一基准；spec 已回写。

### 今日完成

**引擎权限匹配器（metado-engine）**
- `namespace_of`、`permission_allows`（段级匹配：request 为 pattern 段前缀授权；尾段 `*` 贪心覆盖
  `example.com` 多段域名；`*`/`<...>` 段单段通配）、`grants_allow`、`available_namespaces`、
  `exported_namespaces`（命名空间级导出决策 ∪ {metado}）。7 单测。
- 修掉首个版本的命名空间 bug：`http.get.api.example.com` 拆段含 `.` → 尾段 `*` 改贪心后缀。
- C5 回归闭环：请求 `http.get.api.example`（可用 `http.get.api.*`）→ `http` 命名空间不再消失。

**executor 形状化导出 + 方法级放行（plugin_runtime.rs 重写）**
- `PluginRuntime::new(exported, granted, surface, files)`：导出 = 命名空间**对象**（方法为函数，
  `http.get(url)` 形状，规格唯一真）；放行 = 方法级 `ns.method` 对 granted 段级匹配。
- 形状工厂经 `ctx.eval` 建立，权限/形状以 serde_json JSON 嵌入（注入安全），闭包只捕获非 GC 类型。
- 拒绝语义：异步形状（http/storage/file/time.sleep/sha256/hmac/custom/metado）
  → `Promise.reject(PermissionDenied)`；同步形状（log/time.now/crypto.randomBytes）→ 同步 throw。
- `metado` 恒导出、`custom`/`metado` 恒放行（细分由真实实现按 dispatch name 裁决）。
- trace 修复：CapabilityCall 逐方法发射，`granted` 如实传入（不再谎报导出名）；补发射 PermissionCheck。
- async 契约（Important）：`call_default` pump job 队列 + `JsPromise::await_blocking` 读真实状态，
  reject → Err。新增 async 入口测试（返回 42、reject 传播）。
- executor 测试 12→17 全绿；移除 `runtime_namespaces`（由引擎 `exported_namespaces` 取代）。

**cap-file**：`file.readText` → `file.stat`（权限名/导出名/fn 断言对齐规格）。

**CLI/daemon 接线**：env `exported_namespaces` 命名空间级 + `ungranted_requests` 按命名空间判定；
run/test/trace/daemon `PluginRuntime::new(exported, granted, surface, files)` 传入真实 granted；daemon
`exported()` 统一走引擎决策。

**Node dev shim 形状重写**：方法函数形状（`http.get()` 而非 `{__permission, call}`）；`custom={dispatch}`+
`metado={custom: 别名}`；`PermissionDeniedError extends ExecutionError`；`grant` 段级匹配
（`http.get.api.*` 覆盖 `http.get`）；`file.stat`；http 去 `api` 表面导出。`npm test` 全过（12 能力用例）。

**示例/契约断言**：`typeof http === "object"`、`typeof http.get === "function"`；
契约新增 `contract_glob_requested_namespace_still_exported`（C5 回归）。

**spec 回写**：metado 节、custom 改写、导出原则（命名空间级静态面 / 方法级动态面）、权限匹配规则节、
拒绝语义、file.stat 权限名、PermissionDeniedError 命名统一、示例加 metado。

### 排错记录（本次）
1. boa 0.22 `PropertyKey: From<&JsString>` 不实现 → 传 owned `JsString`。
2. `JsValue::as_object()` 返回 owned `Option<JsObject>` → 去掉 `.cloned()`。
3. `serde_json` 未入 executor 依赖 → workspace 依赖补 `serde_json`。
4. env.rs `exported_namespaces` 与 engine 导入同名 → 本地 thin wrapper，删冲突导入。
5. `granted` 移入 `engine.load_plugin` 后借用 → `granted.clone()`。
6. 权限匹配器：`http.get.api.example.com` 拆段（`.` 分隔）→ 尾段 `*` 贪心后缀语义。
7. Node 测试授权清单漏 `log.info` → 授权后同步断言误报。

### 当前状态
- **executor 17/17、engine 全绿、CLI/daemon contract 全绿、Node shim 全绿。**
- 待办：workspace 全量回归（--jobs 1）→ 提交。

### 下一步
- 全量回归 `cargo test --workspace --jobs 1`；`git` 提交。
- 后续评审项（C1 治理、C2 身份闸门、C3 燃料/中断、签名信封字段、容器上限、错误分类、IPC 错误路径）。

---

## 2026-09-21（会话六段） 评审修复：C1 权限治理 + C2 身份闸门 + C3 指令燃料（自决范围）

> 承上段 C5/C4。用户授权自决 → 取评审优先建议 #1（权限/身份治理接线），燃料（C3）作为延伸一并落地。
> 治理决策：**requested（导出面之源）= manifest.permission ∪ permission-set 展开 ∪ 显式 extra；
> engine 为唯一权威（granted ⊆ requested 强制）**；CLI `--grant`/daemon GRANT 是控制面操作，仍受
> requested 上界约束（越界项不生效，而非 400）。

### 今日完成

**C1：granted ⊆ requested 强制（metado-engine 权威）**
- `Plugin` 增 `requested: Vec<String>`（请求全集上限）；`load_plugin` 记录 requested 并按之授予。
- `Engine::grant` 改写：只保留 requested 模板覆盖内的权限（`permission_allows` 段级语义，含模板
  细粒度如 `http.get.api.*` → `http.get` / `http.get.api.example.com`），去重；返回最新 granted。
- permission-set 存在性 + 展开移入 engine：`load_plugin` 展开 `manifest.permission-set`（未定义
  集名 → 加载失败），`requested = 传入 ∪ 展开`。daemon 必须先 `registerPermissionSet` 才能装载
  引集的插件（试验证）。
- 新增访问器 `Engine::requested(plugin_id)` / `Engine::granted(plugin_id)`。
- 治理单测 8（越界过滤/模板内保留/去重/requested 记录/set 展开/未定义集拒绝/替换/冒名）。

**C2：load_plugin 身份闸门**
- 同名加载：signer 相同 → 替换（更新版本，无重复记录）；signer 不同 → 拒绝（防冒名，原插件保留）。

**Host 接线**
- daemon：`load_bundle` 去掉冗余 engine.grant（加载即授）；record.granted = engine 权威 granted；
  GRANT 方法过 engine 过滤后落记录（不再盲 extend）；2 新测试（越界过滤、set 展开/未定义拒绝）。
- CLI：`run.rs` 从 engine 取 granted/requested（含展开）构造执行面；`test.rs`/`trace.rs` 走新辅助
  `env::resolve_requested`（声明 ∪ set 展开 ∪ extra）；未定义 set → 失败。run 增 1 回归测试。

**C3：指令燃料（boa 0.22 fuzz feature）**
- `boa_engine` 启用 `fuzz` feature（新增依赖：`arbitrary` 1.4.2 + `derive_arbitrary` 1.4.1，
  已由用户批准；Cargo.lock 相应更新）。
- `PluginRuntime::new` 以 `DEFAULT_INSTRUCTION_BUDGET=50_000_000` 设 `instructions_remaining`；
  `new_with_instruction_budget` 供测试/细控。紧循环（`for(;;){}`，ExecutionBudget 看不见）
  现被终结为 "instruction budget exhausted" 错误。
- `JsEngine`（旧 executor 桥）同步设预算，避免 fuzz 默认 0 残留导致一切立即失败。
- 已知边界（诚实记录）：`await never-settling promise` 的 await_blocking 仍可挂起
  （job 泵无进展且不消费指令，C3 后续再议中断钩子）。

### 排错记录（本次）
1. fuzz feature 使 `ContextBuilder` 默认 `instructions_remaining=0` → JsEngine 一切立即抛错
   → 统一设预算。
2. 治理测试漏 seed 模板项（load 即授予 requested 本身）→ 断言并入模板。
3. 旧 engine_integration `grant("log.info")` 越界断言 → 改断言"越界被过滤"（新语义即修复）。

### 当前状态
- **workspace 全量回归：64 测试二进制，0 failed，0 warning（--jobs 1，exit 0）。**
- daemon 10/10、engine governance 8/8 + 旧全绿、executor 19/19（含 2 新燃料）、CLI/contract 全绿。

### 下一步
- 提交本轮（C1/C2/C3-fuel）。
- 评审余项：签名信封其他字段、容器大小上限、IPC parse 错误响应、错误分类（FuelExhausted 入
  ErrorKind）、`cmd run` 的 engine granted 与 PluginRuntime 一致性的更宽契约测试。
