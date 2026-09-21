# Metado 全量代码评审

日期：2026-09-20 · 分支 `feat/v1-engine` · 状态：评审结果（待修复闭环）

> 评审范围：全部 11 个 crate + `tests/contract` + `packages/runtime-node` + `examples/*`
> 评审方式：4 个并行评审代理深读源码后汇总，关键结论均已人工对照源码复核
> 基线：`cargo test --workspace` 全量通过（0 failed，0 warning）

---

## 1. 亮点（Strengths）

- **签名门优先且正确**：`Engine::load_plugin` 先 `bundle.verify()` 再解包（`crates/metado-engine/src/lib.rs:55`）；签名恰好覆盖 header+payload，固定偏移解析无溢出路径、零 `unsafe`（`crates/metado-engine/src/signature.rs:50-105`）。
- **CTS 信任底线真实成立**：CLI 全部可执行路径（run/test/trace/env）与 daemon 均在执行前 `SignedBundle::verify()`；未签名容器从不执行（`crates/metado-cli/src/run.rs:42`、`crates/metado-daemon/src/lib.rs:99`）。
- **ZIP 防穿越在入库即生效**：`..` 与绝对路径条目 at-ingestion 拒绝（`crates/metado-engine/src/container.rs:23-25`）。
- **IPC 帧协议分配安全**：4 字节大端长度先校验 ≤ 16 MiB 再分配，无放大攻击面（`crates/metado-ipc/src/transport.rs:62-66`）。
- **六内置能力 crate 划分正确**，权限名已与 Node shim 完全对齐（旧的 `http.fetch`/虚构 crypto 名已修）。
- **契约测试真实执行 ESM** 并守住核心属性：未授权的 `@metado/runtime` import 链接失败（`tests/contract/tests/permission_contract.rs:23-30`）、签名位翻转被拒。

## 2. Critical（5）

1. **`granted ⊆ requested` 从不被执行**。`PermissionResolver`（唯一体现该不变量的组件）是死代码——全仓库仅 re-export（`crates/metado-engine/src/lib.rs:18`），从未被构造。`Engine::grant` 直接 `plugin.granted.extend(perms)`（`lib.rs:93`）、daemon GRANT 同样盲加（`crates/metado-daemon/src/lib.rs:209-223`）。§5.2 核心不变式在所有真实路径上失效。`permission-set` 引用的存在性检查也缺失。
2. **同签名更新强制缺失 + 重名插件静默并存**。`load_plugin` 从不记录 plugin_id → signer 指纹，`plugin()`/`invoke()` 在重名时任意返回第一个（`lib.rs:76-85, 108-115`）。§6.2-3 冒名覆盖防线不存在。
3. **无中断/燃料机制，失控插件不可停**。`ExecutionBudget` 只计边界操作（`crates/metado-executor/src/budget.rs:15-22`），boa 0.22 指令燃料是 `cfg(fuzz)` 才编译（`Cargo.toml:35` 未开）。`while(true){}` 永久挂死事件循环，违背 §9.1/§9.2（fuel → Fault → quarantine）与 §7.6 单线程隔离。
4. **运行时权限拒绝不存在**。`granted` 从未传入 `PluginRuntime`（`crates/metado-daemon/src/lib.rs:144` 只传 exported）；`@metado/runtime` 导出是 `(function(){})` stub（`crates/metado-executor/src/plugin_runtime.rs:63-76`），调用返回 `undefined` 而非 `PermissionDenied` reject；trace 还谎报 `granted: export_names`（`:71`）。
5. **导出形状违反已锁定的 runtime-export-spec**（`docs/superpowers/specs/runtime-export-spec.md`）。规格要求命名空间对象带方法（`http.get(url)`），引擎却按命名空间导出裸函数 stub；Node shim 导出 `{__permission, call}` 对象；示例断言 `typeof http === "function"`（`examples/http-plugin/src/main.js:8`）。三方互相矛盾，规格示例代码在任一运行时都跑不通。

## 3. Important（约 15）

### 引擎核心（metado-engine）

- 签名信封缺 `algorithm` 字段（§6.1/§10.2 要求 `magic + format_version + algorithm + signer_pubkey + payload_len`，现 46 字节布局无其位置）；`format_version` 解析后不校验（`signature.rs:104-126`）；`payload_len` 解读后丢弃。
- zip 启用 deflate（`Cargo.toml:30`）违反 §10.2 "v1 全 store"，`read_to_end` 无大小/总数/条目数上限 → 解压炸弹 OOM（`container.rs:26-28`）。
- 防穿越不完整：只查 `..` 与前导 `/`（`container.rs:23, 45`），反斜杠/盘符/symlink 在 Windows 可绕过；重复条目名静默 last-wins。
- 值模型非单射：`#[serde(untagged)]` 中 Bytes 排在 List 前，`from_json("[1,2]")` 解成 `Bytes([1,2])`（`crates/metado-engine/src/value.rs:6-12`）；NaN/Infinity → `null` 静默丢数据（`:54-60`）。
- 权限仅精确匹配，`http.get` 无法覆盖 `http.get.api.*`，与 executor 的 `starts_with("{name}.")` 前缀逻辑（`crates/metado-executor/src/virtual_module.rs:19-24`）互相矛盾。
- evict → invoke 重建缺失（invoke 直接拒 Evicted，`lib.rs:116-122`）；Quarantine 状态在真实路径不可达。

### 执行器（metado-executor）

- **Async 入口静默返回 Null**：`call_default` 同步调用、不 `run_jobs()`，Promise 被 `js_to_value` 折叠为 `Null`（`plugin_runtime.rs:249-273`）。§4.2/§9.1 的 async 契约整体失效，测试只覆盖 sync。
- 公开 `ModuleLoader` 解析宿主真实文件系统（`crates/metado-executor/src/module_loader.rs:18-38`）是隐患；活动路径 loader 无 Node 风格解析（无扩展名回退 / index.js / exports 字段），`import './util'` 失败（§10.1）。

### 能力 / shim / 示例

- `http.get.api.*` 是死 glob：无 `*` 匹配器，文档中的 `http.get.api.example` 请求会导致 `http` 导出整个消失（`crates/metado-cap-http/src/lib.rs:16`）。
- `<signer>` 绑定只在单测里调用（`crates/metado-engine/src/permission.rs:23`），run/daemon 原样拷贝 `storage.<signer>.write` 字符串 → §6.3 同签互通全线缺失。
- Node shim 缺 `metado` 导出 → `hello-plugin` 第一行 import 在 Node 下链接失败，双运行时承诺（§2）落空。
- shim 无 `storage.<signer>` 权限；file crate/shim 用 `readText`，与规格 `file.stat` 已漂移（有一方过期）。
- §9.2 错误分类实现为零：cap 全部 `Result<_, String>`；Node `PermissionDenied extends MetadoError` 而非 `ExecutionError`（`packages/runtime-node/src/errors.mjs:4-12`），宿主 catch 不到拒绝。
- 无任何容量上限：`crypto.randomBytes(n)`、http/file 响应体、storage.write 全部无界（§8.5 `limit` 类缺失）。
- HTTP 无域名白名单 / 无 SSRF 硬化 / 无超时，raw URL 直达 client（`crates/metado-cap-http/src/lib.rs:45-59`）。

### CLI / IPC / daemon

- JSON-RPC 语法错误被静默 drop（`serve_conn` 里 `continue`，`crates/metado-daemon/src/lib.rs:350-353`），坏客户端永久挂起，无 −32700/−32600 响应；batch 被拒而非回答。
- `mdl run/test` 不接受源目录（§12.2 文档为 `plugin.mdl|dir`），`--ask` 授权模拟未实现（`crates/metado-cli/src/main.rs:127-139`）。
- **daemon REVOKE = UNINSTALL**（`records.remove()`，`lib.rs:224-231`），engine 状态两处都不更新；契约测试只断言 `err.contains("d")`（`tests/contract/tests/alignment_contract.rs:42-52`），形同虚设。
- lifecycle/active 是摆设：invoke 不查状态、engine 从不 activate；setLifecycle 接受任意字符串（`active = state != "unloaded"`）、setDomainConfig 是 no-op（`lib.rs:262-265`）。
- 串行 accept 循环：单连接空闲 30s 会阻塞所有其他客户端，无多客户端并发（`lib.rs:332-340`）。

## 4. Minor（精选）

- 权限层返回 `Err(String)` 而非 `PermissionDenied` kind（§9.2）；`granted` 存原始 `<signer>` 模板产生永不满足的条目。
- `time.sleep` 是阻塞 `std::thread::sleep`（规格为 tokio sleep），会卡住 realm。
- file 约束是词法非规范（symlink 逃逸）；`custom` 三处三种形状；trace `PermissionCheck` kind 定义了但从不发射。
- `mdl verify` 多做了解包 + 解析（规格只要验签），且缺 `--pubkey`；watch 双构建 + 吞错；daemon 纯内存无持久化。
- 大量 `Result<_, String>` 而非 typed 错误，错误分类无从谈起。

## 5. 优先建议（按顺序）

1. **权限/身份治理接线**（Critical 1、2、4）：把 `PermissionResolver` 接入 `Engine`/daemon，落实 `granted ⊆ requested`、revoke 语义、同签名更新闸门——这是设计文档区别于普通运行时的全部价值所在。
2. **执行预算真正可中断**（Critical 3）：给 boa 挂可中断机制（interrupt hook / 每 N 条指令检查标志 / worker 线程 kill），否则 `mdl run` 一个死循环插件即挂死进程。
3. **统一 `@metado/runtime` 导出形状**（Critical 5）：以已锁定的 runtime-export-spec 为唯一基准，改 executor 虚拟模块 + Node shim + 改写示例断言，令规格示例代码两种运行时都能跑。
4. **Async 契约**：`call_default` 补 job-queue pumping，async 入口真实返回。
5. **容量与错误分类**：全部 cap 加字节/元素上限并映射 `ExecutionError` 四类；HTTP 加超时与 domain rules。
6. **IPC 错误路径**：返回 −32700/−32600 响应、batch 明确回答；daemon 并发处理连接。
7. 收尾：签名信封补 `algorithm`/校验 `format_version`、容器解压上限与路径规范化、值模型 Bytes/List 消歧、契约测试真正走 wire 而非 in-process。

## 6. 总体判断

工程结构清晰、测试纪律好、签名/容器/IPC 底层扎实，但**治理语义（权限、身份、预算、async）目前是"占位"而非"实现"**——插件在任一真实路径上都能绕过权限模型、跑死进程、或拿到静默错误。作为 v1 骨架合格，距自身规格定义的"唯一真相"还差 4 项 Critical 的接线与收紧。