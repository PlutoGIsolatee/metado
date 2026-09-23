# Metado 开发环境 (CLI)

## 12. 开发环境（CLI）

### 12.1 定位

- **CLI = 引擎工具链**，是引擎的 in-process 宿主之一（同测试、引擎 daemon 主程序，§13 API 面），不是"另一个引擎"
- **行为对齐**：与生产共用同一引擎核心——验签、权限解析、沙箱语义零差异；`HostApi` 已删，无"CLI 宿主 vs 生产宿主"实现分化，storage/log 等均为引擎同一实现
- **信任底线**：CLI **从不运行未签名容器**（与生产一致）；dev 循环自动以**本机开发密钥**临时签名（生命周期仅本机），保证验签路径在任何模式下都被真实执行

### 12.2 命令总览（分组）

```
# 构建与签名（发布路径）
mdl build <plugin-dir> [--key <sign-key>] [--out <file>]   # 组装 ZIP 容器 + 签名（§10.2）
mdl sign  <plugin.mdl> --key <file>                        # 单独签名
mdl verify <plugin.mdl> [--pubkey <file>]                  # 仅验签（载荷不解包）

# 开发循环（反馈环）
mdl run   <plugin.mdl|dir> [--grant <perm>] [--grant-set <name>]   # 引擎实例执行，模拟生产授权
mdl watch <plugin-dir> [--key <sign-key>]                  # 源变更 → 增量重建 → 热重载 → rerun

# 验证
mdl test  <plugin-dir|plugin.mdl> [--key <sign-key>]       # 插件契约测试

# 诊断与观测
mdl env   <plugin-dir>                                     # 能力/权限出口静态诊断（见 12.5）
mdl trace [<plugin.mdl>] [--filter <evt>] [--json]         # 执行轨迹观测
```

### 12.3 开发循环（watch）

```
编辑源码（合法 Node 项目，任意编辑器/LSP/bundler）
   → 文件变更 → 增量重建 ZIP 容器（store，廉价）→ 开发密钥签名
   → 热重载（复用热引擎，仅重载变更模块/入口，保留住 realm）
   → 自动 rerun（预设入口/测试）
```

- 秒级反馈；实时错误与权限拒绝直接内联提示（借 trace 返回）

### 12.4 授权模拟（run / watch）

- `--grant <perm>` 复刻生产 RPC `grant`（granted ⊆ requested）；`--grant-set <name>` 引用宿主权限集
- 交互 `--ask`：模拟用户削减/确认流（授权弹窗、resident-high 确认），验证插件缺权与升级路径的真实表现
- 生产差异仅剩"授权决策者"：CLI 用 flag/交互代替真实用户/宿主，其余全同

### 12.5 能力/权限静态诊断（env）

- 输出 `requested` 权限清单、`available`（`requested ∩ compiled ∩ active`）后**实际会导出的能力**、入口表、权限集可用性——签名前先看得到"缺了什么、多了什么"
- 呼应 §4.3/§5"未请求的能力导出不存在"：诊断面 = 模块注册静态裁剪的可视化

### 12.6 轨迹观测（trace）

- 事件：入口调用起止 / capability 调用参数与结果 / 每次权限裁决 requested vs granted / 值流转
- 关联调用点（模块 + 行号）供审计定位
- 输出：human 树 + `--json` 供 CI/管道；内部为可复用 trace sink，交互式调试器预留挂接