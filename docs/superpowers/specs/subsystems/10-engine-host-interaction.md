# Metado 引擎-宿主交互

## 11. 引擎-宿主交互（绝对独立进程）

### 11.1 进程原则

- 引擎**始终以独立进程/服务运行**：自洽、故障隔离（插件崩溃不拖垮宿主）、无宿主代码驻留（运行期）
- 引擎产物 = 宿主开发者于构建期定制的独立镜像（§8.1）；宿主运行时**不往引擎进程内注入任何代码**
- 引擎与宿主的唯一运行期交互 = IPC（管理方法面 + 回调面）
- IPC transport 各平台实现：Android bound service、Win/Linux Unix domain socket
- 线格式：**JSON-RPC 2.0**（v1；值传递原则下简单可调试，体积成问题时再二进制编码）
- 本地可信信道（bound service 绑定 / UDS peer 凭据），**引擎-宿主间不加密**：造假面不在本地管道，插件签名面才是外界信任边界

### 11.2 管理方法面（host → engine）

| 方法 | 语义 |
|---|---|
| `loadPlugin(bytes)` | 验签闸门 → 解析 → 静态检查 → 返回 `{plugin_id, signer_id, requested, lifecycle}` |
| `grant / revoke(plugin_id, perms)` | 授予（granted ⊆ requested）、削减 |
| `setLifecycle(plugin_id, mode)` | 设 effective 常驻模式（升级超出承诺由宿主负责先确认） |
| `invoke(plugin_id, entry, value)` | 调入口，`Result<Value, ExecutionError>`；插件内排队（§7.6） |
| `listPlugins / uninstall / purge` | 生命周期管理（uninstall 默认保留 storage） |
| `registerPermissionSet(name, perms)` | 定义权限集 |
| `setDomainConfig(ruleId, json)` | 领域规则运行时配置（白名单/配额/降级） |
| `getState / setState` | 读写引擎持久化状态（engine/state.json） |
| trace 订阅 | 流式轨迹开/关 |

- **引擎从不等待用户**：任何需人确认的动作（安装、resident-high、授权）= 宿主自己弹 UI → 事后调 RPC（grant/setLifecycle）

### 11.3 回调面（engine → host）

| 回调 | 语义 |
|---|---|
| `dispatch(name, params, requestId)` → host 返回 Value/Error | **capability message**：app 驻留服务收到引擎路由的值，执行后 resolve 插件 promise（v1 单次 Request-Response，逐次请求） |
| `notify(event)` | 事件流：`realmEvicted` / `quarantined(plugin_id)` / `pluginUpdated` / `pluginRemoved` |

### 11.4 权威划分

- **引擎 = 无主见执行者 + 状态所有者**：代码执行、权限裁决、生命周期、签名验签、沙箱、内置能力机制（模块在引擎内自给自足）、**持久化状态（engine/state.json）、单写者、原子替换、文件锁、自主状态落盘**
- **宿主 = 决策权威 + 命令源 + 观测**：信任决策（安装/削减/升级确认）、管理配置（白名单/配额/密钥供给）、**不直接读写 state**、app 驻留服务（经 capability message）、观测消费
- 领域规则 v1 为引擎内配置驱动（setDomainConfig），不做宿主实时回调；将来需要宿主实时的走 `dispatch` 通道（§8.4）

### 11.5 值、错误、信任线

- 全链路同一值模型（§4.4）；RPC 序列化 JSON 兼容 + Bytes → `Uint8Array`
- `ExecutionError { entry, kind, message }` 原样跨 RPC 回宿主
- 插件签名信任不依赖 IPC（验签在引擎内、载荷进引擎前完成），宿主只管理"信不信该 signer"（§6）