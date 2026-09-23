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
- **workspace 全量回归：64 个测试套件结果（52 测试二进制 + 12 doc-test），0 failed，0 warning（--jobs 1，exit 0）。**
- daemon 10/10、engine governance 8/8 + 旧全绿、executor 19/19（含 2 新燃料）、CLI/contract 全绿。
- **已提交：`531112c`**（fix(runtime): C1 permission governance + C2 signer gate + C3 instruction fuel，
  17 文件，+526/−35）。

### 下一步
- 评审余项：签名信封其他字段、容器大小上限、IPC parse 错误响应、错误分类（FuelExhausted 入
  ErrorKind）、`cmd run` 的 engine granted 与 PluginRuntime 一致性的更宽契约测试、C3 中断钩子
  （never-settling await）。

## 会话七：处置台账核验（disposition audit）落地（2026-09-21）

对 `docs/superpowers/2026-09-21-disposition-audit.md`（对处置台账的对码核验）逐条处置，
核实 4 条声明后全部属实并修复：

1. **3.2 `<signer>` 模板方法面缺口**（真）：`build_ns_methods` 取 `split('.')[1]`，
   `storage.<signer>.read` 的方法名退化为字面 `<signer>`（此前被 cap-storage 同时注册裸
   `storage.read/write` 掩盖）。**RED** 新增 `test_signer_template_surface_yields_bare_methods`
   （surface/granted 仅含 `<signer>` 形式 → 期望 `read,write/function/function`，实测
   `<signer>/undefined/undefined`）→ **GREEN** 改为跳过 `<...>`/`*` 占位段取首个真实段。
2. **3.3 `PermissionResolver` 死代码**（真）：全 crate 无生产调用，语义已被
   `permission_allows`/`grants_allow` + engine 权威 grant 取代。删除 struct、`lib.rs` re-export
   及 5 个专属测试（保留 `PermissionSet` 测试）。
3. **3.4 trace `requested` 字段近似**（真）：`CapabilityCall.requested` 实填导出命名空间名。
   改名为 `exported` 并加文档注释（engine/executor/cli 三处），消除误导。
4. **3.1 计数措辞**（口径问题）：原「64 测试二进制」不精确；更正为「64 个测试套件结果
   = 52 测试二进制 + 12 doc-test」。

- **全量回归（本轮后）**：`cargo test --workspace --jobs 1` → 64 套件全 ok，**214 passed，
  0 failed，0 warning，exit 0**（较上轮 218 passed：+1 新增、−5 删除的 resolver 测试）。
- 未改审计文档本体；审计结论「台账基本准确、诚实」维持。

---

## 2026-09-22 设计单元：引擎可替换与状态所有权（对话式设计评审）

与用户逐条对话，确立"引擎整体可替换 + 引擎持有状态"的增补设计；**产出新文档**
`docs/superpowers/specs/2026-09-22-engine-replaceable-store-design.md`（未提交）。

### 确立的关键结论

- **权威两轴**：决策权威始终宿主（命令源）；**状态所有权归引擎**（选 3）。宿主永不直接读写
  store，只经 IPC 命令/查询（store 对宿主黑盒）。
- **可信域**：宿主 + 引擎 + 引擎数据目录（含 store）同一可信域，store **不签名不加密**；
  防的是**插件**篡改（能力面 + 文件树双隔离）；信任决策不落 store（是宿主决策非引擎事实）。
- **store 文档模型**：自描述文档、**命名空间 + 字段**（命名空间 = 引入该字段的引擎分支标识符，
  branch-id 唯一性暂不约束）；统一字段规则——认得按语义用、**不认得原样保留绝不据此拒绝**、
  只加不改、主版本不认拒绝启动（唯一硬门闩）、原子替换（tmp→rename）、单写者（启动文件锁）。
- **物理分离**：governance（引擎自身，独立原子文件机制）与 plugin-data 区（低层存储原语 +
  数据）**物理与机制分离**；低层持久化原语**非唯一**、供内置能力复用（storage 不特殊），
  **具体形态与具体内置 API 一并推后**。
- **生命周期**：三态 absent / installed / uninstalled；`granted` 是**纯放行集非运行门槛**
  （**granted 空也可运行**，未授能力调用 PermissionDenied）；pending 取消；可运行性 = installed。
- **`active`（引擎级）运行时不可变**：为引擎产物启动配置，不进 store，运行时 `available`
  不可变（setActive 从管理面移出）。
- **update 语义**：同签校验；`granted ← 旧 ∩ 新` 自动延续 + 新请求项需显式 grant（同签
  不能偷扩权）；重装还原旧私有卷数据。
- **SemVer 门闩**：仅插件 manifest version 强制 SemVer，**只管理"是否执行更新"**（>旧允许、
  ==拒绝、<拒绝走 uninstall+install），不承载授权/兼容语义；允许 prerelease。
- **权限模型归正**：三层不变，**逐次裁决归引擎执行层**，不存在"宿主执行引擎授权"；"宿主侧
  二次裁决"表述废止（改"引擎执行层裁决（CLI 内联）"）。
- **CLI/dev**：`mdl run` 走同一 governance store 机制（工作目录，可 `--workdir`）；生产差异
  只剩"授权决策者"。

### real-execution-chain 计划修正两点（写入新文档 §11）
1. "宿主侧二次裁决"归正为"引擎执行层裁决（CLI 内联）"；
2. CLI 落工作目录 store。

### 当前状态
- 新增文档（未提交）：`docs/superpowers/specs/2026-09-22-engine-replaceable-store-design.md`
（待评审）。设计讨论待续：可回写主文档（rev 11）或按评审结论再修订。
- **未写任何代码**；既有代码基线不变（214 passed 维持）。

### 下一步
- 用户评审新文档；定稿后决定是否同步修订主文档（rev 10 §7/§8.5/§11.2/§11.4）与
  real-execution-chain 计划（两处措辞）。
- 待补：branch-id 唯一性机制、低层持久化原语形态 + 具体内置 API（一并推后）。

---

## 2026-09-22 命名重构与归档收尾（对话式设计评审完结）

在用户评审通过 `2026-09-22-engine-replaceable-store-design.md` 后，完成全文命名重构、旧文档归档、LOG 补记，设计评审正式完结。

### 命名替换记录（全文批量替换）

| 旧术语 | 新术语 | 说明 |
|---|---|---|
| `store` / `store.json` | `state` / `state.json` | 引擎状态文档，去“存储”歧义 |
| `governance` / `governance store` | `engine` / `engine/state.json` | 引擎自身数据目录与文件 |
| `plugin-data` / `plugin-data 区` | `plugins/` + `shared/`（顶层） | 插件数据域拆分：私有归属插件、共享归属签名者 |
| `plugin-data/private/` | `plugins/<id>/private/` | 私有 keyspace 归属插件目录 |
| `plugin-data/shared/` | `shared/`（顶层） | 共享域跨插件，提升到顶层 |
| `bundles/`（平级） | `plugins/<id>/bundles/` | 插件本体归入插件目录 |
| `store.json` | `state.json` | 状态文档 |
| `__meta__.json` | `meta.json` | 无双下划线，跨平台安全 |
| `__ownership__.json` | `ownership.json` | 同上 |
| `store`（泛指） | `state` | 全文替换，含变量/路径/注释 |
| `governance`（泛指） | `engine` | 全文替换 |
| `plugin-data`（泛指） | `plugins/` / `shared/` | 全文替换 |

### 核心语义修正（同步落笔）

1. **可重算数据一律不持久化**：`signer_id`/`version`/`requested`/`entries` 全从 `.mdl` 重算；state 仅留决策事实（granted/domain config/lifecycle effective）、引用（bundle_ref/volume_ref）、状态键。
2. **同签判据改为“当前存储本体比对”**：无首装指纹锚，直接与当前存储本体重算 signer 比对；主文档 §6.2-3 字面据此修订。
3. **`active` 运行时不可变**：改为引擎产物启动配置，不进 state，运行时 `available` 不可变。
4. **`granted` 空也可运行**：pending 取消；可运行性 = installed；放行 = granted。
5. **`active` 不进 state**：移出管理面 `setActive`，改为引擎产物启动配置。

### 归档动作

1. **设计文档归档**：`docs/superpowers/specs/2026-09-21-real-execution-chain-design.md` 顶部加弃用/归档标记，指向新文档。
2. **实现计划归档**：`docs/superpowers/plans/2026-09-21-real-execution-chain.md` 顶部加弃用/归档标记，指向新文档。
3. **新基线文档**：`docs/superpowers/specs/2026-09-22-engine-replaceable-store-design.md`（未提交，待评审通过后作为增补设计留存，不合入主文档）。

### 评审结论

- 新文档评审通过，**作为独立增补文档留存，不合入主文档**。
- 旧 `real-execution-chain` 设计/计划文档已归档，仅供留档参考。
- 后续执行以新设计文档为准，实现计划将另行编写。

### 当前状态

- 新设计文档：`docs/superpowers/specs/2026-09-22-engine-replaceable-store-design.md`（未提交，评审通过待落笔）。
- 旧设计/计划文档：已加归档标记。
- 代码基线不变（214 passed 维持）。
- 下一步：编写新实现计划（基于新设计文档），按 TDD 落地。

---

## 2026-09-23 设计文档完结与子系统拆分

### 本次完成工作

**1. 主设计文档全量修订（rev 10 → rev 11）**
- 将 `docs/superpowers/specs/2026-09-22-engine-replaceable-store-design.md` 确认的增补设计全量合入主文档 `2026-09-18-metado-design.md`（rev 10 → rev 11）
- 核心变更已在 `2026-09-22` 日志中记录：state持久化、术语统一、生命周期三态、同签判据改为当前本体重算、active不可变、可重算数据不持久化、同签判据改为当前本体重算、CLI/dev走同一state机制

**2. 主设计文档子系统拆分（14个子系统文档）**
在 `docs/superpowers/specs/subsystems/` 下创建 14 个子系统文档 + 索引：
- 01-overview.md / 02-architecture.md / 03-core-concepts.md / 04-permissions.md
- 05-signature-trust.md / 06-lifecycle.md / 07-host-capabilities.md
- 08-execution-errors.md / 09-packaging-build.md / 10-engine-host-interaction.md
- 11-cli.md / 11-engine-api.md / 12-scope-phases.md / 13-deferred.md
- README.md（子系统索引 + 修订记录表）

**3. 旧文档归档**
- `2026-09-21-real-execution-chain-design.md` → 顶部加弃用标记，指向新设计
- `2026-09-21-real-execution-chain.md` (plan) → 顶部加弃用标记
- 删除 `2026-09-21-real-execution-chain-conformance.md`

**4. 文档库结构更新**
```
docs/superpowers/specs/
├── 2026-09-18-metado-design.md (rev 11, 主设计)
├── 2026-09-22-engine-replaceable-store-design.md (增补，不合入主文档)
├── 2026-09-21-real-execution-chain-design.md (归档)
└── subsystems/ (14 子系统 + README.md 索引)
```

### 当前状态
- 主设计文档 rev 11 已落盘（`2026-09-18-metado-design.md`）
- 14 子系统文档 + README.md 已落盘（`specs/subsystems/`）
- 旧设计/计划文档已归档标记
- 代码基线不变（214 passed 维持）
- 新增设计文档 `2026-09-22-engine-replaceable-store-design.md` 评审通过，作为独立增补文档留存，不合入主文档

### 下一步
- 编写新实现计划（基于 rev 11 主设计 + 子系统文档），按 TDD 落地
- 计划覆盖：engine/state.json 读写+锁、state 文档模型、plugins/shared/bundles 目录管理、low-level persistence primitive trait、PluginRuntime::new_with_state、CLI run_mdl 接线、示例/测试、回归
