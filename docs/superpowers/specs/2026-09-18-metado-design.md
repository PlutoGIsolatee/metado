# Metado 设计文档

日期：2026-09-18（rev 3：二进制 ESM 模块树容器，生产可调试）
状态：待评审

## 1. 概述

Metado 是一个**可嵌入、跨平台（Android / Windows / Linux）的 JavaScript 运行时**，配套**受治理的单文件插件打包格式**。插件用标准 JavaScript（ESM）编写，计算内核可用 WebAssembly，以带签名的单文件 `.mdl`（**二进制 ESM 模块树容器**）交付，由引擎加载执行。

Metado 的核心价值是**细粒度权限治理**：插件级最小权限、用户可设置的授予、领域相关的极细分权限、权限集机制，再加 **Android 式签名信任体系**（完整性验签 + 签名即身份 + 同签更新 + 同签名数据互通）。

一句话定位：**一个能装进移动应用的小型、纯洁、可静态验证、带细粒度权限治理与签名信任的 JS 运行时。**

## 2. 目标与非目标

### 目标

- 跨平台（Android / Windows / Linux）可嵌入，引擎为独立进程/服务
- 语言 = **标准 JS（ESM）**，可混合 WASM 计算内核，且两者高效进程内集成
- **插件源码 = 合法 Node 项目**（除 API）：标准 npm 结构（package.json + node_modules）、Node 工具链直接可用；**唯一例外**——Node 核心 API（process/fs/net/child_process/require…）运行时不可用，一律以受治理的 metado 能力替代（仅白名单 shim：Buffer/path/events）
- **能力 API = 统一模块 `@metado/runtime`（跨运行时契约）**：引擎内为内置虚拟模块，Node 内为真实 npm 包，同一源码双运行时；**目标状态**：Node 可运行 + 现有工具链完全可测试（napi 完整等价后端，实现推迟见 §15）
- 细粒度权限治理作为一等公民：请求 / 授予 / 执行三层语义
- **分级生命周期常驻治理**：声明 / 用户确认 / 可修改（resident-high / resident-low 默认 / cold），正确性不依赖内存态
- 接口约束（类型契约）加载期静态校验
- Android 式签名体系：完整性验签、签名即稳定身份、同签更新、同签名数据互通
- 单文件发布（**二进制模块树容器，零打包转换**），多文件源码
- **生产可读可调试**：分发物保留真实模块结构与源码（无 minify/混淆/bundle 转换），堆栈指向真实文件与行号
- CLI 开发环境与生产环境行为对齐（单一引擎核心），快速迭代与热重载

### 非目标

- 任何自定义脚本语法 —— 插件就是标准 JS
- 代码转换/最小化/混淆 —— 分发物零转换、零 minify（TS 编译推迟，见 §15）
- TypeScript —— v1 源码 = 纯 ESM JS，.ts 不支持（推迟，见 §15）
- 引擎裁决"发布者可不可信" —— 信任决策在用户/宿主侧，引擎只验证不裁决
- 插件之间跨插件调用（v1 不支持）
- 交互式单步调试器（v1 只做热重载 + 轨迹观测）
- 句柄式宿主对象跨入 WASM 沙箱（永久不变量）
- 密钥轮换（同签更新缺失时的迁移路径，需升级签名 scheme 时再设计）

## 3. 架构总览

```
                     ┌──────────────────────────────────────────┐
                     │            引擎核心（唯一）                │
                     │  容器/Manifest 解析│ 单元模型│ 类型系统    │
                     │  权限解析器 │ 签名验签 │ 轨迹观测(Trace)   │
                     └───┬──────────┬──────────┬──────────┬──────┘
                         │          │          │          │
                     JS 执行器(boa)│ WASM 执行器(wasmi)│ 宿主能力注册表│ Capability 路由器
                         │          │          │          │
                     ┌───┴──────────┴──────────┴──────────┴──────┐
                     │       引擎进程（daemon/service）            │
                     │   进程间通信层（IPC transport）              │
                     └───┬──────────────────┬────────────────────┘
                         ▼                  ▼
               ┌───────────────┐    ┌─────────────────┐
               │ 生产宿主（app）│    │ CLI 宿主（dev）  │
               │ 真后端+真实授权 │    │ mock host + trace│
               └───────────────┘    └─────────────────┘
```

- **引擎核心**：单一实现，CLI 与生产共用，保证行为对齐
- **宿主 API 抽象**（`HostApi` trait）：CLI 提供 std 宿主（可配置 mock），生产宿主实现真实后端
- **自定义能力**：应用定制能力以类型化 capability message 路由到应用进程

## 4. 核心概念

### 4.1 插件形态

插件 = **标准 ESM JavaScript 代码 + 结构化 manifest 元数据**，源码侧是一个**合法 Node 项目**（除 API），交付为一个签名的二进制单文件容器：

```
plugin.mdl =
  签名信封（signer_pubkey, signature, algorithm）      §6
  模块树容器（manifest + 索引 + ESM 模块文件 + wasm）  §10
```

### 4.2 入口单元（Unit）

单元 = manifest 声明的**入口导出**（指向标准 ESM 导出），附带类型契约与权限条目。**只有声明的入口单元是引擎可见、可治理的**。

源码侧 manifest（`mdl.toml`，TOML）：

```toml
name       = "myplugin"
version    = "1.0.0"
permission = ["http.get.api.example", "storage.<signer>.write"]   # 插件级请求权限
permission-set = ["standard"]                                      # 引用宿主定义权限集

[units.onMessage]
type   = "Json->Json"      # 接口契约
export = "onMessage"       # 指向 ESM 导出名

[units.transform]
type   = "Bytes->Bytes"
export = "transform"
```

对应 JS（标准 ESM）：

```js
// on_message.js
export async function onMessage(input) { ... }
// transform.wasm 对应字节由 manifest 引用内联
```

- **可见性语义由 ESM 天然承担**：未在 manifest 声明的导出是纯内部实现，引擎不可见、不可调用
- 所有 manifest 声明（单元、契约、权限）在**加载期静态校验**（`type=Json->Json` 等）

### 4.3 内部模块组织

- 单元内可 `import` 本插件内部 helper 模块与纯 JS npm 包，**完全通用标准 ESM**
- 容器挂载为**模块文件系统**，boa 模块解析器按原始 `import "./helper.js"` 直接解析 —— **零打包、零转换**，模块图即文件树
- 内部模块不受权限治理、不出现在 manifest 入口中，不暴露为可调用单元 —— 只有入口单元带契约与权限

### 4.4 类型契约（接口约束）

- `type=x->y` 中的类型名为**引擎级统一类型标识**，均指向引擎的 Rust 宿主类型
- 核心类型：`String / Number / Bool / Bytes / Json / List<T> / Any`
- 宿主可注册自定义类型（名字 + serde 实现）
- **连接校验：严格相等**。调用方期望类型与单元声明类型严格一致，只开放 `Any` 作为显式逃生口
- 运行时校验单元输出与声明类型一致，不一致即失败

### 4.5 WASM 值边界（架构不变量）

> **句柄/对象身份永不跨入 WASM 沙箱。** WASM 单元永远是纯函数式值进出。

- WASM 单元只能接收/返回普通值（经线性内存搬运），无句柄、无对象身份、无生命周期管理
- 好处：离线可测、沙箱最紧、唯一资源管控 = fuel 计量 + 输入大小
- 需要长期状态的场景放 JS 层；WASM 只管纯计算内核
- wasmi 作为 v1 运行时（纯 Rust 解释器，内置 fuel 计量，跨平台零障碍）。API 镜像 wasmtime，未来可替换

## 5. 权限模型（核心）

三层要素：

```
① 请求（manifest 声明）           ② 授予（宿主/用户运行时决定）              ③ 执行（每次调用检查）
permission = [...]                 engine.grant(plugin, [...])             host_api 调用 →
 [http.get.api.example,             // granted ⊆ requested                    resolver.check(
  storage.<signer>.write]            // 用户可削减，领域规则叠加                    requested, granted,
                                     // engine.define_permission_set(...)         domain_rules) → pass/deny
```

### 5.1 权限语法（点分式）

- 细粒度点分式：`http.get`、`http.get.api.example`、`storage.read.profile`、`log.info`
- **signer 域引用**：`storage.<signer>.write`、`vfs.<signer>.read` 等 —— `<signer>` 在加载期绑定为该插件实际的 signer_id（§6），构成**同签名互通**的显式授权面
- 声明位置：manifest 顶层 `permission` 字段（插件级）

### 5.2 三层语义

- **最小权限**：realm 授权视图（加载时解析）= `requested ∩ available`；未请求的能力导出不存在（访问即快速失败）。静态可审计，默认拒绝
- **用户可设**：`granted ⊆ requested`，运行时由宿主应用决定实际授予范围；每次宿主 API 调用经运行时执行层复核
- **领域规则**：宿主可注册领域级 resolver 叠加（如"http 仅限本应用白名单域名"、"调用次数超限自动降级"）

### 5.3 权限集

- 宿主应用定义命名权限集，插件引用：

```rust
engine.define_permission_set("standard", ["http.get", "log.info"]);
engine.define_permission_set("finance",  ["http.get.api.bank", "storage.read", "crypto.sign"]);
```

- 插件 manifest：`permission-set = ["standard", "finance"]`
- 插件内联定义权限集仅用于内部复用
- 加载期检查：引用的权限集必须已由宿主定义，否则加载失败

## 6. 签名与信任（类 Android 模型）

### 6.1 机制

- **算法：Ed25519（v1 唯一）**；签名 64 字节，验签快，Rust 生态成熟。无 RSA/ECDSA/X.509/证书链 —— 信任判定在引擎外，PKI/CA 无必要
- **信封格式**：

```
.mdl = header { magic, version, signer_pubkey, algorithm = "ed25519" }
     + signature（覆盖后续所有字节）
     + payload（模块树容器：manifest + 索引 + 模块文件 + wasm 原始字节）
```

- **`signer_id` = 公钥指纹**（如 sha256 截段），作为插件的稳定发布者身份

### 6.2 语义（复制 Android 结构，简化机制）

1. **载荷前独立验签闸门**：`load_plugin` 先验签（甚至不解包 payload），失败即拒
2. **签名 = 稳定身份**：signer_id 绑定插件身份
3. **同签更新（引擎强制）**：同 plugin_id 的新 bundle，`signer_pubkey` 指纹必须与原安装记录一致，否则拒绝 —— 防冒名覆盖，不依赖用户决策
4. **引擎不裁决可信性**：只做完整性验证 + 身份比对；发布者可不可信由用户/宿主决定（对应 Android 的渠道/侧载）
5. **私钥永不入引擎**：签名私钥只在插件开发者侧，文件只含公钥与指纹

### 6.3 同签名互通（显式 opt-in）

- 以 signer 域划分存储/vfs 命名空间的第二层：

```
storage keyspace:
  私有   storage:<plugin_id>:<key>      ← 本插件专属
  共享   storage:<signer_id>:<key>      ← 同发布者（同签）共享

权限声明显式 opt-in：
  storage.<signer>.write / vfs.<signer>.read
  未声明 → 共享域不可达；同签插件默认不互通，最小权限保持
```

- 引擎需在解包验签后、权限注入前完成 `<signer>` 绑定
- 共享域基于签名验证后的 signer_id，不可伪造（验签是前置闸门）

## 7. 插件生命周期

### 7.1 开发者契约（正确性不依赖内存态）

- **任意一次 invoke 都可能跑在全新的 realm 上**（宿主可能在两次调用间回收了它）
- 显式 storage keyspace 是**唯一真相**；realm 内存态是缓存（best-effort），随时可丢
- 可选 **boot 钩子**：realm 每次创建时触发一次，用于从 storage 重水合
- 更新（同签新版本）→ realm 必然重建，旧 realm 回收

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

### 7.4 生命周期状态机

```
absent → installing（验签，记录 plugin_id + signer 指纹）→ installed
       → loading（解包 / 静态检查 / realm 创建）→ pending（等待宿主授予）
       → active ⇄ evicted（host evict → 冷；invoke → 重建 realm + boot 重水合）
       → quarantine（沙箱崩溃 / fuel 耗尽致 realm 不可用；宿主可 reload，引擎进程不崩）
       → active'（update 同签新版本，realm 必然重建）
任何状态 → uninstalled（撤销 grants + 移除插件；storage 默认保留，见 7.5）
```

### 7.5 卸载语义

- 卸载**默认保留私有 storage**（重装即还原数据），持久化价值保留
- 显式清除可选：宿主 API / `mdl purge` 按插件清除（满足用户隐私诉求）
- 共享 signer 域不随单个插件清理

### 7.6 并发

- **插件级单线程**：同一插件同一时刻仅一个 invoke，任务排队（head-of-line 由插件自身 await 行为承担）
- **跨插件并行**：独立 realm → 相互隔离、可并行；v1 单线程协作调度多 realm，将来 realm+任务队列移入 worker 线程是内部优化，对 API 无感

## 8. 宿主能力与能力框架（SDK 构建）

### 8.1 引擎 = 平台 + 能力框架（按需构建实现）

- **定制构建定位**：每个宿主构建自己的引擎进程镜像（源码构建），进程隔离保留（§11）
- **crate 布局**（feature 矩阵的答案）：

```
metado-engine        # 核心 crate：能力框架开放、单元模型、权限、执行器、值类型
metado-cap-http      # 内置能力 = 独立 crate/feature，按需取舍
metado-cap-storage   #   各自的重依赖只进需要它的构建
metado-cap-...       #   storage/vfs/file/time/log/crypto
metado-cli / 绑定    # 工具与宿主绑定
```

- 宿主：`default-features = false, features = ["metado-cap-storage"]` + 自研能力 crate，组合矩阵按 crate 隔离而非 feature 网格
- **开放能力契约（源码级扩展性）**：`#[metado::capability(set = "...")]` 是导出给宿主 crate 的公开宏 / `impl CapabilitySet` trait，一个 impl 产出——权限名表 + 宿主函数集 + JS 绑定（`@metado/runtime` 导出结构）+ trace 元数据 + IPC 路由描述 + 领域规则钩子
- 宿主自研能力与内置能力**完全同待遇**：最小权限注入、运行时裁决、CLI trace、行为对齐
- 双层裁剪：**编译期 features** 消除重依赖（TLS/crypto 等），**运行时 `.with()`** 组合激活；`available = registered ∩ compiled`

### 8.2 内置能力集（built-in capability sets）

- 独立 crate/feature：`http / storage / vfs / file / time / log / crypto`（storage/vfs 支持 signer 命名空间）
- 初版即采用"默认零依赖最小核 + 宿主按需叠加"的构建方式

### 8.3 三类能力分工

| 层 | 位置 | 用途 |
|---|---|---|
| 内置能力集 | metado 仓库，宿主 features 选择 | http/storage/vfs/crypto 等通用能力 |
| **宿主源码级能力集** | 宿主 crate，`#[capability]` | 宿主定制的原生能力，随引擎镜像进引擎进程 |
| capability message（8.4） | 跨 IPC | 真正住在 app 进程的服务（UI 等），引擎路由过去 |

- 宿主源码级能力**默认原生运行于引擎进程**；需要 app 侧参与时，其实现可把调用转发为 capability message（引擎→app）

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
- **未授权绑定不存在**：realm 授权视图只含 `granted ∩ available`，访问未授权项快速失败（两种运行时同一语义）
- 「无 import 样板」的自动绑定注入**放弃**（Node 下不存在自由标识符注入，统一 import 才能保证单源码双运行时）

### 8.6 HostApi 抽象

`HostApi` trait 是引擎与宿主能力的唯一接口。CLI 的 std 宿主与生产宿主都实现它，保证行为一致。

## 9. 执行与错误处理

### 9.1 执行

- JS 单元：入口导出为 **async 函数（input → output）**，boa 驱动，引擎单一事件循环推进 job queue
- WASM 单元：同步式调用（值经线性内存进出），执行使用 fuel 计量，不干扰事件循环
- 宿主按名调用入口单元：`plugin.invoke(unit_name, input)`
- 值边界统一：值跨 JS/WASM/宿主任意边界使用同一套引擎级值表示（引擎值 model）

### 9.2 错误处理

**一概短路中止，无错误作为值的传播。**

- 加载/静态期错误（格式非法、签名无效、权限声明非法、类型不衔接、引用未授权 API、权限集未定义）在 `load_plugin` 时 fail-fast
- 运行期错误 → 结构化 `ExecutionError { unit, name, kind: Runtime|Permission|Sandbox, message }`，由宿主决定处置
- 沙箱/环境错误（fuel 耗尽、WASM trap、序列化失败）永远中止
- 插件作者需要"失败即结果"的场景，在 JS 层用 try/catch 自行消化

## 10. 打包与构建

### 10.1 源码（合法 Node 项目，标准 JS 结构）

插件源码是一个**合法 Node 项目**（除 API，§2）：结构、约定与工具链兼容 npm 生态，运行时 API 由 metado 治理能力面代替 Node 核心 API。

```
myplugin/
  package.json          # name/version/"type":"module"/exports
  package-lock.json     # 依赖快照（可复现构建）
  node_modules/         # npm install 产出，真实 npm 依赖
  mdl.toml              # manifest：name/version/permission/permission-set/units/lifecycle
  src/
    on_message.js       # 标准 ESM：export async function onMessage(input){...}
    helpers/util.js     # 内部模块，自由组织
  wasm/
    transform.wasm      # 或 transform.wat/source（构建时编译）
```

- 结构 = 标准 npm 项目：`npm install` 直接工作，编辑器/lint/LSP/bundler 全部可用
- 唯一例外是运行时 API：**Node 核心内置模块不可用**（process/fs/net/child_process/require），一律以受治理的 metado 能力替代；仅白名单 shim（`Buffer`、`path`、`events`）
- 运行时模块解析为 **Node 风格**：`exports` field 优先、`node_modules` 逐级查找（§9.1/阶段 2）

### 10.2 分发（单文件，签名）

- **容器格式**：二进制模块树容器 = 索引（入口表）+ 原样 payload（模块文件、wasm 原始字节）。含 manifest section、ESM 模块树、wasm 二进制。可压缩；索引支持快速定位
- `mdl build` 流程：**组装容器**（把源码树原样装入，零转换/零 minify）→ **`mdl sign --key <file>` 签名** → 输出单文件 `plugin.mdl`
- 模块级可读性不因容器格式受影响：加载后仍是文件树，堆栈指向真实路径与行号
- 曾考虑的纯文本容器（base64 wasm / 转义）**放弃**：可读性需求在加载后的模块树层面，二进制容器更小、更快、免转义

### 10.3 生态复用（npm 包）

- **插件就是合法 Node 项目**，npm 依赖是标准手段：`npm install` 产出 node_modules，`mdl build` 按 `package-lock.json` 快照把依赖拷入容器（可复现），不手动管理
- 纯 ES module JS 包可拷入容器（lodash-es、axios、zod 等），模块原样保留
- **不做自动 `require()`→ESM 重写**；CJS-only 包需作者在自有工具链中预转换（否则不支持）
- 提供轻量 Node API shim（`Buffer`、`path`、`events`）
- 依赖 Node 原生模块的包不可用，必须用宿主 API 替代（`import { storage, http } from "@metado/runtime"`）

## 11. 进程模型与通信

- 引擎为独立进程/服务，**镜像由宿主定制构建**（§8.1）：故障隔离（插件崩溃不拖垮宿主）、宿主语言解耦、沙箱升级路径
- IPC transport 抽象层各平台实现：Android bound service、Win/Linux Unix domain socket
- 协议线格式：**JSON-RPC 2.0**（v1 默认；值传递原则下简单可调试，若体积成为问题再引入二进制编码）
- 协议边界均为值传递；宿主源码级能力原生运行于引擎内（零 IPC），app 侧服务走 capability message

## 12. 开发环境（CLI）

```
mdl build <plugin-dir> [--key <sign-key>]     # 组装模块树容器 + 签名
mdl run     <plugin.mdl> [--grant http.get]    # std 宿主执行，可配置 grants 模拟生产
mdl watch                                      # 监听源文件 → 增量重建容器 → 热重载 → 自动 rerun
mdl test                                       # 单元/集成测试（宿主 API 契约测试）
mdl trace                                      # 执行轨迹观测
mdl sign     <plugin.mdl> <key>                # 单独签名/验签
```

- **热重载**：复用热引擎，重载变更单元，秒级反馈循环
- **轨迹观测（Trace）**：单元起止 / 宿主 API 调用参数与结果 / 每次权限裁决 requested vs granted / 值流转；设计为可复用观测 API，供未来交互式调试器挂接
- **行为对齐**：CLI 与生产共用同一引擎核心；验签、权限解析、类型契约、沙箱语义零差异

## 13. 引擎 API 面（Rust，示意）

```rust
let mut engine = Engine::new();
engine.register_capability(Http::default());        // / storage / file / time / log / crypto
engine.register_type::<MyType>("MyType");            // 可选：注册自定义宿主类型
engine.define_permission_set("standard", ["http.get", "log.info"]);

let plugin = engine.load_plugin("plugin.mdl").await?;   // 验签闸门 → 解析 → 静态检查 → 权限注入
let signer = plugin.signer_id();                        // 稳定发布者身份（公钥指纹）
let mode   = plugin.lifecycle();                        // 读 effective 常驻模式（宿主可设）
engine.grant(&plugin, ["http.get.api.example"]);        // 宿主授予（可被用户削减，granted ⊆ requested）

let out = plugin.invoke("onMessage", json!({...})).await?;
// Result<Value, ExecutionError>
```

更新验签：引擎按 plugin_id 保存首次安装的 `signer_pubkey` 指纹，后续同 plugin_id 的新 bundle 验签比对（§6.2-3）。

## 14. 范围分解与阶段

本设计由多个子系统组成，实现按阶段推进：

1. **引擎核心（metado-engine）**：签名验签、容器/manifest 解析（非自定义语言语法）、单元模型、类型系统、权限解析器、值表示、错误模型，以及**开放的能力框架（`#[capability]` 宏 + `CapabilitySet` trait、.with() 注册）**（无 JS/WASM 执行）
2. **JS 执行器**：boa 桥接、**Node 风格模块解析**（exports field / node_modules 逐级）、模块系统挂载（容器文件树）、**`@metado/runtime` 虚拟内置模块映射**、值互转、事件循环集成
3. **WASM 执行器**：wasmi 桥接、原始字节加载、import 函数、fuel 计量
4. **内置能力集（metado-cap-*）**：能力框架落地 + http/storage/vfs/file/time/log/crypto（含 signer 命名空间），独立 crate/feature
5. **CLI 工具（mdl）**：build（容器组装 + 签名）/ run / watch / test / trace，std 宿主，npm 拷入
6. **引擎进程 + IPC**：transport 抽象、capability message 路由、平台实现（Android/Win/Linux）
7. **示例与契约测试**：宿主自研能力示例（源码级扩展性验证）、行为对齐验证、CLI/生产对比测试
8. **（推迟）Node 运行/测试包**：`@metado/runtime`（引擎侧已按规格供给）+ `metado-node`（napi 完整等价后端）、`@metado/testing` 测试架势

## 15. 明确推迟项

- 交互式单步调试器（trace API 预留扩展点）
- **Node 运行/测试包（推后）**：`@metado/runtime` 的 Node 侧真实供给 + `metado-node`（napi-rs 链 `metado-core` 的完整等价后端，真实权限裁决同源 Rust）、`@metado/testing`。**v1 不做**（纯 JS fallback 也不做）：需每平台原生构建矩阵 + V8/boa 语义对齐（由阶段 7 契约测试兜底）；但「能力 API = `@metado/runtime` 模块统一规格」的形态 v1 即遵循，引擎侧先行供给
- **TypeScript**：v1 源码 = 纯 ESM JS；`.ts` 编译暂不加入 `mdl build`（需先定编译器选型 swc-rs/esbuild、sourcemap 恢复 .ts 行号，并修订"零转换"承诺，契机再启）
- wasmtime（wasmi 镜像其 API，性能成为刚需时切换）
- 跨插件调用
- 零拷贝共享 buffer（`Bytes` 优化）
- 压缩（容器初版可不压缩，体积成为问题时再启用，分节内透传）
- 密钥轮换（同签更新缺失时的迁移路径，需升级签名 scheme 时再设计）
- 句柄式宿主对象跨 WASM 沙箱（永久拒绝）