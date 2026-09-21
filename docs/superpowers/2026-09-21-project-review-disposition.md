# Metado 评审处置台账（2026-09-20 评审 → 09-21 修复闭环）

日期：2026-09-21 · 分支 `feat/v1-engine`
来源：`docs/superpowers/2026-09-20-project-review.md`（全部 11 crate + contract + runtime-node + examples）
格式：逐项处置 = 状态 + 出处提交 + 备注

> 状态图例：✅ 已修复并回归 · 🟡 部分（主体完成，剩余明确列出）· ⬜ 未处理（列入待办）
> 相关提交：`336c8b9` = C5 导出形状 + C4 运行时放行 + Async；`531112c` = C1 权限治理 + C2 身份闸门 + C3 指令燃料；`1404d68` = contract 加固（更早）；`603a9bf` = 日志。

## 1. Critical（5）

| # | 项 | 状态 | 提交 | 备注 |
|---|----|------|------|------|
| C1 | `granted ⊆ requested` 从不执行（PermissionResolver 死代码、daemon GRANT 盲加、permission-set 存在性缺失） | ✅ | `531112c` | engine 为唯一权威：`Plugin.requested` 上界、`Engine::grant` 段级过滤 + 去重并返回最新 granted、set 展开（未定义集 → 加载失败）；daemon GRANT/load、CLI run/test/trace 全部接线。治理测试 8 个 |
| C2 | 同签名更新缺失 + 重名静默并存 | ✅ | `531112c` | `load_plugin` 同名闸门：signer 相同 → 替换（无重复记录）；不同 → 拒绝（防冒名，原插件保留） |
| C3 | 无中断/燃料机制，`while(true){}` 挂死 | 🟡 | `531112c` | 指令燃料（boa 0.22 `fuzz` feature，`instructions_remaining`）：紧循环被终结为 "instruction budget exhausted"。**遗留**：① never-settling `await` 仍会挂住 `await_blocking`（不消费指令，燃料管不到）② Fuel→Fault→quarantine 错误分类链未接 ③ worker 线程 kill / interrupt hook 未做 |
| C4 | 运行时权限拒绝不存在（granted 未入 PluginRuntime、导出是裸 stub、trace 谎报 granted） | ✅ | `336c8b9` | `PluginRuntime::new(exported, granted, surface, files)`；异步形状 reject / 同步形状 throw；trace 的 CapabilityCall 如实传 granted，补发射 PermissionCheck |
| C5 | 导出形状违反已锁定 spec（engine/shim/示例三方矛盾，规格代码跑不通） | ✅ | `336c8b9` | 导出为命名空间对象 + 方法函数（`http.get(url)`）；Node shim 重写同形；示例断言 `typeof http === "object"` / `typeof http.get === "function"`；spec 回写为唯一基准（含 metado 节、custom=dispatch、file.stat） |

## 2. Important（~15）

### 引擎核心（metado-engine）

| 项 | 状态 | 备注 |
|----|------|------|
| 签名信封缺 `algorithm`、`format_version` 不校验、`payload_len` 丢弃 | ⬜ | 待办：§6.1/§10.2 信封布局 |
| zip deflate 违反 v1 store；`read_to_end` 无大小/条目上限（解压炸弹） | ⬜ | 待办：容器容量上限 |
| 防穿越不完整（反斜杠/盘符/symlink；重复条目 last-wins） | 🟡 | `..`/绝对路径入库即拒已加固（`1404d68`）；Windows 路径形式与重复条目警告未做 |
| 值模型非单射（Bytes/List untagged 冲突、NaN/Infinity→null） | ⬜ | 待办 |
| 权限仅精确匹配 vs executor 前缀矛盾 | ✅ | `permission_allows` 段级匹配（尾段 `*` 贪心）：`http.get` 覆盖 `http.get.api.*`，双方统一同语义 |
| evict → invoke 重建缺失；Quarantine 不可达 | ⬜ | 待办：生命周期恢复路径 |

### 执行器（metado-executor）

| 项 | 状态 | 备注 |
|----|------|------|
| Async 入口静默返回 Null | ✅ | `call_default` 补 job-queue pump + `JsPromise::await_blocking` 读真实状态；测试覆盖 await 值/reject |
| unused `ModuleLoader` 解析宿主真实文件系统；活动 loader 无 Node 风格解析 | ⬜ | 待办：路径解析（无扩展名回退 / index.js / exports） |

### 能力 / shim / 示例

| 项 | 状态 | 备注 |
|----|------|------|
| `http.get.api.*` 死 glob（请求 `http.get.api.example` 致 http 导出消失） | ✅ | 命名空间级导出决策 + 段级贪心 glob；契约回归 `contract_glob_requested_namespace_still_exported` |
| `<signer>` 绑定只在单测 | 🟡 | 匹配器支持 `<...>` 段单段通配（grant 过滤器可用）；"同签互通"具体 signer 替换绑定链路未接 |
| Node shim 缺 `metado` 导出 | ✅ | `metado.custom`（custom.dispatch 别名），恒导出 |
| shim/file 用 readText 与规格漂移 | ✅ | 统一 `file.read` / `file.stat`；Node shim 与 cap-file 同步 |
| 错误分类实现为零（cap 全 `Result<_, String>`；Node 错误层次错） | 🟡 | Node `PermissionDeniedError extends ExecutionError` 已修；Rust cap 仍 `Result<String>`，`FuelExhausted` 入 `ErrorKind` 未做 |
| 容量上限（crypto/http/file/storage 无界） | ⬜ | 待办：§8.5 limit |
| HTTP 无域名白名单/SSRF 硬化/超时 | ⬜ | 待办：domain_rules 字段未接线 |

### CLI / IPC / daemon

| 项 | 状态 | 备注 |
|----|------|------|
| JSON-RPC 语法错误静默 drop；batch 被拒 | ⬜ | 待办：−32700/−32600 响应、batch 回答 |
| `mdl run/test` 不收源目录、`--ask` 未实现 | ⬜ | 待办 |
| daemon REVOKE = UNINSTALL；engine 状态两处不更新；契约断言弱 | ⬜ | 待办：revoke 真撤销语义；`alignment_contract.rs` `err.contains("d")` 断言补强 |
| lifecycle/active 摆设（invoke 不查状态、setLifecycle 任意字符串、setDomainConfig no-op） | ⬜ | 待办：状态机真实接线 |
| 串行 accept 循环阻塞其他客户端 | ⬜ | 待办：并发处理连接 |

## 3. Minor（精选）

| 项 | 状态 | 备注 |
|----|------|------|
| 权限层 `Err(String)` 而非 `PermissionDenied` kind | 🟡 | Node 分类修复；Rust 侧仍 `String` |
| granted 存原始 `<signer>` 模板产生永不满足条目 | 🟡 | 匹配器已能通配 `<...>`；模板原文仍可能存于 granted |
| `time.sleep` 阻塞线程（规格为 tokio sleep） | ⬜ | 待办 |
| file 约束词法非规范（symlink 逃逸） | ⬜ | 待办 |
| `custom` 三处三种形状 | ✅ | 统一 `dispatch(name, params)` + `metado.custom` 别名，spec 锁定 |
| trace `PermissionCheck` 从不发射 | ✅ | 逐方法发射，capability = `ns.method` |
| `mdl verify` 多做解包 + 解析 | ⬜ | 待办：仅验签 |
| watch 双构建 + 吞错 | ⬜ | 待办 |
| daemon 纯内存无持久化 | ⬜ | 待办（明确为 v1 范围外亦可） |
| 大量 `Result<_, String>` 无 typed 错误 | 🟡 | 同上错误分类 |

## 4. 优先建议对照

| 序 | 建议 | 状态 |
|----|------|------|
| 1 | 权限/身份治理接线（C1、C2、C4） | ✅ C4（`336c8b9`）、C1+C2（`531112c`）；revoke 语义除外（见上） |
| 2 | 执行预算真正可中断（C3） | 🟡 指令燃料完成；interrupt hook / worker kill / 永不 settle await 待办 |
| 3 | 统一 `@metado/runtime` 导出形状（C5） | ✅ `336c8b9` |
| 4 | Async 契约 | ✅ `336c8b9` |
| 5 | 容量与错误分类 | ⬜ |
| 6 | IPC 错误路径 | ⬜ |
| 7 | 收尾（签名信封 / 容器上限 / 值模型 / 契约走 wire） | ⬜ |

## 5. 结论与剩余待办簇

评审结论「治理语义是占位而非实现」的核心缺口已闭环：**Critical 全项有修复提交**，且每项都带 TDD 测试与全量回归（workspace 64 测试二进制，0 failed，0 warning）。剩余工作聚为四簇：

1. **容量与错误分类**：cap 上限 + `ExecutionError` 四类映射 + FuelExhausted kind（评审建议 5）
2. **IPC 服务健壮性**：错误响应 / batch / 并发（评审建议 6）
3. **签名信封与容器**：algorithm 字段、format_version 校验、store 压制、解压上限、路径规范化（评审建议 7）
4. **生命周期语义**：revoke 真撤销、invoke/active/lifecycle 接线、evict 重建、C3 中断钩子

（`docs/superpowers/LOG.md` 会话六段另有关键决策与排错记录。）