# Metado 执行与错误处理

## 9. 执行与错误处理

### 9.1 执行

- **入口 = JS async 导出函数（input → output）**：boa 驱动，引擎单一事件循环推进 job queue
- **WASM 计算内核 = 内置 API**（§4.5，实现推迟）：标准 `WebAssembly` 命名空间 + 直接 `.wasm` 导入双面，同一实现内核，执行计入同一预算，错误映射见 §9.2
- 宿主按名调用入口：`plugin.invoke(entry_name, value)`
- 值边界统一：值跨 JS / 宿主 / IPC 使用同一套引擎值表示（§4.4）

### 9.2 错误处理

**边界守卫：`invoke` 结果永远是「成功 Value」或「`ExecutionError`」，错误本体不入值模型**（值模型无 Error 类型，输出契约纯 JSON 兼容值）。插件期待的"业务失败"用 JS try/catch + 返回约定结构（如 `{ok:false, code, message}`，仅约定，不新增 API）。

- **载荷/静态期错误**（格式非法、签名无效、权限声明非法、权限集未定义、静态导入 `available` 之外的能力导出 = ESM 链接失败）→ `load_plugin` fail-fast；管理面（load / 管理 RPC）失败 = JSON-RPC error 对象，与 invoke 分线
- **运行期错误** = `ExecutionError { entry, kind, message }`——kind 四故障域；**引擎自动判 realm 存亡，宿主只判业务处置**：

| kind | 实例 | realm | 宿主处置 |
|---|---|---|---|
| `PluginError` | JS 抛错 / unhandled rejection / WASM trap | 存活 | 展示 / 重试 |
| `PermissionDenied` | granted 缺 / domain rule 拒（revoke 后调用、动态 import 兜底） | 存活 | 提示补授 |
| `CapabilityError` | storage 写失败 / http 5xx / app 侧 capability message 失败 | 存活 | 按能力处置 |
| `Fault` | fuel 耗尽 / 序列化失败 / 容量超限 | **引擎判定**（fuel→quarantine §7；序列化/容量→存活） | 只收 `notify` |

- **权限拒绝的 JS 形状统一**：能力函数返回 promise，`PermissionDenied` 一律 reject 该 promise（boa 与 Node 同一）；缺权 ≠ 缺导出——导出照 `available`，调用才裁决（§8.5）
- 沙箱判定归属引擎：fuel/trap 经 §7 quarantine 通知宿主，宿主不判 recover
- 错误事件进 trace（含 requested vs granted 上下文）