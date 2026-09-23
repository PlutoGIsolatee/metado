# Metado 插件生命周期

## 7. 插件生命周期

### 7.1 开发者契约（正确性不依赖内存态）

- **任意一次 invoke 都可能跑在全新的 realm 上**（宿主可能在两次调用间回收了它）
- 显式 storage keyspace 是**唯一真相**；realm 内存态是缓存（best-effort），随时可丢
- 可选 **boot 钩子**：realm 每次创建时触发一次，用于从 storage 重水合
- 更新（同签新版本）→ realm 必然重建，旧 realm 回收
- **引擎持久化状态（engine/state.json）实现自洽恢复**：启动读取 state，重建插件表、授权、配置，不依赖宿主输入

### 7.2 Realm 生命 = 宿主可回收的运行时资源

- active 插件持有温 realm；宿主策略可随时 **evict**：内存压力、空闲超时、Android 生命周期、插件长时间未用
- 驱逐后再次 invoke = 重建 realm → boot 重水合 → 执行
- 温 realm 只是**性能缓存，不是语义**；三种常驻模式只是性能档位，正确性契约对全部一致

### 7.3 分级常驻模式（请求 / 授予 / 执行）

manifest 顶层声明（请求）：

```toml
lifecycle = "resident-high"   # 或 "resident-low"（默认）| "cold"
```

| 模式 | 语义 |
|---|---|
| `resident-high` | 温 realm 常驻，仅极端内存压力下驱逐；冷调延迟最低 |
| `resident-low`（默认） | 便宜时温着，压力下最先驱逐；有条件的常驻 |
| `cold` | 从不保持温 realm，每次 invoke 重建 + boot 重水合；占用仅容器本体 |

治理规则（与权限体系同构）：

- **声明 = 请求**；`resident-high` 是资源承诺 → 安装时需用户确认；`cold` 零承诺，无需确认
- effective 模式用户/宿主**随时可修改**：降级随意；**升级超出当前承诺需重新确认**（如 low→high）
- 运行时只看 **effective 模式**做驱逐决策，不看声明
- engine/宿主 API：`plugin.lifecycle()` 读 effective；宿主设置 effective；用户改为 `cold` → 立即驱逐 realm

### 7.3 状态模型（修订）

```
absent → installed（记录入 state；即刻可 invoke）
       → uninstalled（记录移除；数据默认保留，purge 删）
granted ⊆ requested：纯放行集合，非运行门槛（granted 空也可运行）
运行态全在 realm 层：无 realm / 温 / evicted / quarantine
```

- **可运行性 = installed；放行 = granted**。“能跑但没权限”常态，非异常。
- `pending` 状态**取消**（仅作展示标签"未授任何权限"，不载执行语义）。
- 权限语义归正：**缺权 ≠ 缺导出**（导出照静态面，调用才裁决）；插件装完即可 invoke，未授能力的能力调用一律 `PermissionDenied`。

### 7.5 生命周期状态机

```
absent → installing（验签，记录 plugin_id + signer 指纹）→ installed
       → loading（解包 / 静态检查 / realm 创建）→ active ⇄ evicted（host evict → 冷；invoke → 重建 realm + boot 重水合）
       → quarantine（沙箱崩溃 / fuel 耗尽致 realm 不可用；宿主可 reload，引擎进程不崩）
       → active'（update 同签新版本，realm 必然重建）
任何状态 → uninstalled（撤销 grants + 移除插件；storage 默认保留，见 7.5）
```

### 7.6 卸载语义

- 卸载**默认保留私有 storage**（重装即还原数据），持久化价值保留
- 显式清除可选：宿主 API / `mdl purge` 按插件清除（满足用户隐私诉求）
- 共享 signer 域不随单个插件清理

### 7.7 并发

- **插件级单线程**：同一插件同一时刻仅一个 invoke，任务排队（head-of-line 由插件自身 await 行为承担）
- **跨插件并行**：独立 realm → 相互隔离、可并行；v1 单线程协作调度多 realm，将来 realm+任务队列移入 worker 线程是内部优化，对 API 无感