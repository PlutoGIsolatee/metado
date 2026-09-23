# Metado 宿主能力与能力框架

## 8. 宿主能力与能力框架（SDK 构建）

### 8.1 引擎 = 平台 + 能力框架（宿主开发者定制引擎）

- **定制引擎定位**：引擎是平台/SDK；**宿主开发者**在构建期选择内置能力（features）并编译自己的能力集，产出**宿主定制的引擎产物**——该产物是独立进程
- **运行期自洽**：定制只发生在构建期；运行期引擎进程内无宿主代码驻留（绝对独立进程，§11），宿主交互仅经 IPC
- **crate 布局**（feature 矩阵的答案）：

```
metado-engine        # 核心 crate：能力框架开放、入口/模块注册、权限、执行器、值模型
metado-cap-http      # 内置能力 = 独立 crate/feature，按需取舍
metado-cap-storage   #   各自的重依赖只进需要它的构建
metado-cap-...       #   storage/vfs/file/time/log/crypto
metado-cli / 绑定    # 引擎工具链与宿主 IPC 绑定
```

- 宿主开发者：`default-features = false, features = ["metado-cap-storage"]` + 自研能力 crate，组合矩阵按 crate 隔离而非 feature 网格
- **开放能力契约（源码级扩展）**：`#[metado::capability(set = "...")]` 是导出给**宿主开发者** crate 的公开宏 / `impl CapabilitySet` trait，一个 impl 产出——权限名表 + 宿主函数集 + JS 绑定（`@metado/runtime` 导出结构）+ trace 元数据 + capability 路由描述 + 领域规则钩子（配置驱动）
- 定制能力与内置能力**完全同待遇**：最小权限注入、运行时裁决、trace、行为对齐
- 双层裁剪：**编译期 features** 决定"能用什么"，**引擎产物内运行时激活**（构建期配置）；`available = compiled ∩ active`，**`active` 为引擎产物启动配置，运行时不可变，不进 state**

### 8.2 内置能力集（built-in capability sets）

- 独立 crate/feature：`http / storage / vfs / file / time / log / crypto`（storage/vfs 支持 signer 命名空间）
- 初版即采用"默认零依赖最小核 + 宿主按需叠加"的构建方式

### 8.3 三类能力分工

| 层 | 位置 | 用途 |
|---|---|---|
| 内置能力集 | metado 仓库，宿主开发者 features 选择 | http/storage/vfs/crypto 等通用能力 |
| **宿主开发者定制能力集** | 宿主开发者 crate，`#[capability]`，构建期 | 宿主定制的原生能力，编译进宿主定制的引擎产物 |
| capability message（8.4） | 跨 IPC | 真正住在 app 进程的服务（UI 等），引擎路由过去 |

- 定制能力经构建期进入引擎产物后随引擎进程**自洽运行**（运行期无宿主代码驻留）；需要 app 侧参与时走 capability message（引擎→app）

### 8.4 自定义能力（capability message）

- 应用定制能力（UI、业务服务）以**类型化能力消息**提供，不走函数调用/句柄
- 插件调用：`const r = await metado.custom("ui.alert", {title, body})`（`metado` 来自 `@metado/runtime` 导入）
- 引擎把值路由给应用订阅的 capability channel；应用执行后可选返回一个值 → resolve 为 promise，插件 `await` 继续
- 边界纪律：跨进程永远传值，不传句柄；每类 message 有 schema，可版本化

### 8.5 JS 侧 API 访问面

- 能力 API 的唯一入口 = 统一模块 **`@metado/runtime`**（跨运行时契约）：
  - 引擎（boa）：模块加载器把该 specifier 解析为**内置虚拟模块**，按当前 realm 授权视图提供导出
  - Node（开发/测试）：解析为真实 npm 包，同一 specifier、单一源码双运行时
- 插件写法：`import { http, storage, metado } from "@metado/runtime"`
- **导出存在性 = 静态面、调用放行 = 动态面**：导出按 `available`（requested ∩ compiled ∩ active）静态裁剪，realm 创建定格；`granted` 运行时可削减，**导出不随 revoke 重建 realm**，已授后被削的调用走运行期 `PermissionDenied`（reject promise，两种运行时同一语义，§9.2）
- **可运行性 = installed，放行 = granted**：`granted` 空也可运行，仅作逐能力调用的放行集合；`pending` 状态取消
- **载荷传递 = 整值 + 容量上限**（v1 不支持流式宿主 API，§15）：能力入参出参为完整 Value；实施默认 + 宿主 domain rules 配额约束 Bytes/元素上限，超限 = `ExecutionError{kind=limit}`；大 blob 尽量留在引擎内（内置 storage 不跨 IPC），需逐期诱导增量时为推迟的流式能力后置
- 「无 import 样板」的自动绑定注入**放弃**（Node 下不存在自由标识符注入，统一 import 才能保证单源码双运行时）

#### API 风格原则（形状镜像，语义自研）

- **Web Platform 优先**：标准形状直接镜像（fetch 风格 http、WebCrypto 风格 crypto、URL / Blob / ArrayBuffer / TextEncoder 等共享类型）—— JS 开发者成本最低；同一 facade 文件双运行时（V8 原生即含大部分）
- **Node whitelist shim 保持 Node 形状**（Buffer/path/events，为 npm 互操作而存在）
- **无标准可依处局部自研最小面**（storage 的 signer 命名空间 keyspace、vfs、time/log、capability dispatch）
- **只镜像形状，不镜像权限语义**：每次调用仍过 realm 授权视图；shim 只承诺形状兼容，不承诺 Node 行为
- **具体导出清单未锁定** → 研究待办：逐一对照 Web/Node 约定确定初版导出面（§14）

### 8.6 引擎-宿主交互面（替代旧 HostApi）

宿主没有 in-process 代码面（**无 `HostApi` trait**）。引擎进程自洽，宿主经 IPC 交互（§11）：

- **管理方法面（host→engine）**：grant/revoke、setLifecycle、invoke、listPlugins、uninstall/purge、registerPermissionSet、setDomainConfig、trace 订阅
- **回调面（engine→host）**：capability message dispatch（app 驻留服务）+ 事件流 notify
- **引擎内置能力自给自足**（机制在引擎内），宿主只经管理面配置/治理；可选 Provider 覆盖经 capability message（慢通道、显式启用）