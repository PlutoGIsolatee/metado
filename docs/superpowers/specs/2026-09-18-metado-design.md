# Metado 设计文档

日期：2026-09-18（rev 10：引擎绝对独立进程、宿主开发者定制引擎、入口模型）
状态：待评审

> **声明：本文所有实现示例（Rust 示意、manifest/TOML 示例、RPC 方法名、capability 形状、API 用法）仅供示意，保证接口语义一致即可；具体命名与形态以实现为准。**

## 1. 概述

Metado 是一个**可嵌入、跨平台（Android / Windows / Linux）的 JavaScript 运行时**，配套**受治理的单文件插件打包格式**。插件用标准 JavaScript（ESM）编写，计算内核可用 WebAssembly，以带签名的单文件 `.mdl`（**二进制 ESM 模块树容器**）交付，由引擎加载执行。

Metado 的核心价值是**细粒度权限治理**：插件级最小权限、用户可设置的授予、领域相关的极细分权限、权限集机制，再加 **Android 式签名信任体系**（完整性验签 + 签名即身份 + 同签更新 + 同签名数据互通）。

一句话定位：**一个能装进移动应用的小型、纯洁、可静态验证、带细粒度权限治理与签名信任的 JS 运行时。**

## 2. 目标与非目标

### 目标

- 跨平台（Android / Windows / Linux）可嵌入，**引擎绝对独立进程/服务**（宿主进程内无引擎代码、引擎进程内无宿主代码，运行期）
- 语言 = **标准 JS（ESM）**，可混合 WASM 计算内核（内置能力，v1 后落地 §4.5），且两者高效进程内集成
- **插件源码 = 合法 Node 项目**（除 API）：标准 npm 结构（package.json + node_modules）、Node 工具链直接可用；**唯一例外**——Node 核心 API（process/fs/net/child_process/require…）运行时不可用，一律以受治理的 metado 能力替代（仅白名单 shim：Buffer/path/events）
- **能力 API = 统一模块 `@metado/runtime`（跨运行时契约）**：引擎内为内置虚拟模块，Node 内为真实 npm 包，同一源码双运行时；**目标状态**：Node 可运行 + 现有工具链完全可测试（napi 完整等价后端，实现推迟见 §15）
- 细粒度权限治理作为一等公民：请求 / 授予 / 执行三层语义
- **分级生命周期常驻治理**：声明 / 用户确认 / 可修改（resident-high / resident-low 默认 / cold），正确性不依赖内存态
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
                     │           引擎核心（唯一实现）              │
                     │  容器/Manifest 解析│ 入口注册│ 值模型       │
                     │  权限解析器 │ 签名验签 │ 轨迹观测(Trace)    │
                     └───┬──────────┬──────────┬──────────┬──────┘
                         │          │          │          │
                      JS执行器(boa)│能力注册表│Capability 路由器
                 （WASM 内核 metado-cap-wasm：内置能力，推迟实现，v1 后 §4.5）
                         │          │          │
                     ┌───┴──────────┴──────────┴──────────────┐
                     │  引擎进程（daemon/service，绝对独立）       │
                     │  IPC（JSON-RPC 2.0，双向）                  │
                     └───┬──────────────────┬────────────────────┘
                         │ 管理方法面        │ 回调面（capability message + 事件）
                         ▼                  ▼
                ┌───────────────┐   ┌──────────────────┐
                │ 宿主（app）    │   │ CLI（引擎工具链）  │
                │ 管理+服务订阅   │   │ 独立实例+trace    │
                └───────────────┘   └──────────────────┘
```

- **引擎进程绝对独立**：自洽运行、故障隔离、宿主进程内无任何引擎代码，亦无宿主代码驻留引擎进程（§11）
- **引擎核心**：单一实现，CLI 与生产共用同一引擎，保证行为对齐
- **宿主交互仅经 IPC**：管理方法面（host→engine）+ 回调面（engine→host：capability message / 事件流）

## 4. 核心概念

### 4.1 插件形态

插件 = **标准 ESM JavaScript 代码 + 结构化 manifest 元数据**，源码侧是一个**合法 Node 项目**（除 API），交付为一个签名的二进制单文件容器：

```
plugin.mdl =
  签名信封（signer_pubkey, signature, algorithm）      §6
  ZIP 模块树容器（manifest + src + node_modules + wasm）  §10
```

### 4.2 入口（Entries）

插件对外暴露 = manifest 声明的**入口**（`入口名 → ESM 导出`）。**只有声明的入口被宿主调用**；权限是插件级的（§4.3）。

源码侧 manifest（`mdl.toml`，TOML）：

```toml
name        = "myplugin"
version     = "1.0.0"
permission  = ["http.get.api.example", "storage.<signer>.write"]  # 插件级请求权限
permission-set = ["standard"]                                     # 引用宿主定义权限集

[entries.onMessage]
export = "onMessage"       # 指向 ESM 导出名

[entries.boot]             # 可选：realm 创建时触发一次（重水合，§7.1）
export = "boot"
```

对应 JS（标准 ESM）：

```js
// on_message.js
export async function onMessage(input) { ... }
// boot.js
export async function boot() { ... }
```

- 入口是**纯 async 函数**：宿主传引擎值（§4.4），JS 侧映射标准 JS 值；**无 manifest 类型声明**，接口由 JS 语言自身承担
- **可见性语义由 ESM 天然承担**：未在 manifest 声明的导出是纯内部实现，引擎不可见、不可调用
- 所有 manifest 声明（入口、权限）在**加载期静态校验**
- `boot` 入口在 realm 首次触达前执行，用于从 storage 重水合；无 boot 声明则直接执行

### 4.3 内部模块组织

- 插件代码可 `import` 本插件内部 helper 模块与纯 JS npm 包，**完全通用标准 ESM**
- 容器挂载为**模块文件系统**，boa 模块解析器按原始 `import "./helper.js"` 直接解析 —— **零打包、零转换**，模块图即文件树
- **权限是插件级声明**（manifest 顶层），realm 授权视图作用于整个插件——内部模块与入口**同过同一权限解析器**，无任何绕过通道：helper 里 `import { http } from "@metado/runtime"` 照常走 `granted ∩ available` 裁决
- 内部模块**不出现在 manifest 入口、不作为可调用入口**：入口是可调用面（权限是插件级的）

### 4.4 值模型（Value Model）

- 引擎统一值表示：`Null / Bool / Number / String / Bytes / List / Json-Map`（JSON 兼容 + Bytes）—— 值跨 JS / WASM / 宿主任意边界同一表示
- 入口只传值：宿主 invoke 传 Value，JS 侧映射标准 JS 值；JS 侧无法序列化的对象（函数/句柄/循环引用）不跨边界
- **无 manifest 级类型声明**：接口由 JS 语言自身与宿主侧期望承担；"接口是什么"是文档/测试层的事（`mdl test`），引擎不掺和
- Bytes 为明确值类型：跨 IPC 与 `Uint8Array`/`ArrayBuffer` 对应
- WASM 内核的类型化 IO 是内核自身契约（§4.5），不由 manifest 声明

### 4.5 WASM 计算内核（能力契约；**实现推迟 v1 后**）

> **句柄/对象身份永不跨入 WASM 沙箱。** WASM 只做纯函数式计算内核，由插件 JS 实例化调用。

- **WASM = 内置能力 `metado-cap-wasm`**：与 http/storage 同待遇，插件经能力治理使用；它不是独立入口
- 容器内 .wasm 字节由插件 JS 加载并实例化，JS 负责编排（`onMessage` 里调内核做数值计算）
- 进出值只能是普通值（线性内存搬运）：无句柄、无对象身份、无生命周期；需要长期状态的场景放 JS 层
- fuel 计量约束内核执行；离线可测、沙箱最紧，唯一资源管控 = fuel + 输入大小
- **v1 纯 JS**：引擎无 WASM 运行器；较重计算走宿主定制能力 / capability message，JS 防失控用 boa 执行预算（interrupt / 递归 / 栈限制）
- **v1 后实现**：wasmi 桥接（纯 Rust 解释器、内置 fuel、跨平台零障碍）、容器字节加载、插件 JS 实例化调用、值进出无句柄；性能成为刚需时切 wasmtime（API 预留镜像）
- JS 面 = 标准 `WebAssembly` 命名空间（Web Platform 形状，§8.5）：boa 若自带支持则复用；否则引擎自托管 wasm 运行器提供该全局；能力缺失时全局为桩/缺失
- 加载路径为开放实现点（该能力实现时核验，§14）

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
.mdl = header { magic "MDL1", format_version, signer_pubkey, algorithm = "ed25519" }
     + signature（覆盖后续所有字节）
     + payload（ZIP 模块树容器：manifest + src + node_modules + wasm，§10.2）
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
- 双层裁剪：**编译期 features** 决定"能用什么"，**引擎产物内运行时激活**（构建期配置，可经 RPC 管理面调整）；`available = compiled ∩ active`

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
- **未授权绑定不存在**：realm 授权视图只含 `granted ∩ available`，访问未授权项快速失败（两种运行时同一语义）
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

## 9. 执行与错误处理

### 9.1 执行

- **入口 = JS async 导出函数（input → output）**：boa 驱动，引擎单一事件循环推进 job queue
- **WASM 计算内核由插件 JS 加载实例化**（§4.5）：fuel 计量、值进出、不单独作为入口
- 宿主按名调用入口：`plugin.invoke(entry_name, value)`
- 值边界统一：值跨 JS/WASM/宿主任意边界使用同一套引擎值表示（§4.4）

### 9.2 错误处理

**一概短路中止，无错误作为值的传播。**

- 加载/静态期错误（格式非法、签名无效、权限声明非法、引用未授权 API、权限集未定义）在 `load_plugin` 时 fail-fast
- 运行期错误 → 结构化 `ExecutionError { entry, kind: Runtime|Permission|Sandbox, message }`，由宿主决定处置
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
  mdl.toml              # manifest：name/version/permission/permission-set/entries/lifecycle
  src/
    on_message.js       # 标准 ESM：export async function onMessage(input){...}
    helpers/util.js     # 内部模块，自由组织
  wasm/
    transform.wasm      # 计算内核原始字节（.wat 由作者侧工具先行编译，非引擎转换）
```

- 结构 = 标准 npm 项目：`npm install` 直接工作，编辑器/lint/LSP/bundler 全部可用
- 唯一例外是运行时 API：**Node 核心内置模块不可用**（process/fs/net/child_process/require），一律以受治理的 metado 能力替代；仅白名单 shim（`Buffer`、`path`、`events`）
- 运行时模块解析为 **Node 风格**：`exports` field 优先、`node_modules` 逐级查找（§9.1/阶段 2）

### 10.2 分发（单文件，签名）

**plugin.mdl = 签名信封 + ZIP 模块树容器**

```
plugin.mdl =
  签名信封
    header       magic "MDL1" + format_version(u16) + algorithm("ed25519")
                 + signer_pubkey(32B) + payload_len(u64)
    signature    Ed25519(64B)，覆盖 header 之后全部字节（验签闸门在解包前，§6.2）
    payload      ZIP 模块树容器

payload = ZIP（entry = 文件，路径 = 容器内相对路径；v1 全 store，不压缩）
  mdl.toml           # manifest：name/version/permission/permission-set/entries/lifecycle
  src/**             # ESM 模块，原样字节
  node_modules/**    # npm 依赖，原样字节
  wasm/**            # WASM 计算内核，原样字节
```

- **索引 = ZIP central directory**：任意模块按 path O(1) 定位 offset/length，无需自写二进制索引表；**入口表 = manifest（entries）**，容器内无冗余索引
- **工具链可检视**：`unzip`/标准工具直接打开（契合"合法 Node 项目"），生产调试仍读加载后的文件树与真实行号
- **免转义**：wasm / 任意字节原样入 entry，零 base64
- **零转换**：store 即原样字节；压缩推迟（§15），将来 per-entry DEFLATE 不破坏格式（header 含 format_version 供演进）
- **挂载安全**：只读模块文件系统，路径按容器内相对路径解析，防 `../` 穿越
- `mdl build`：组装 ZIP 容器（源码树原样装入）→ `mdl sign --key <file>` 签名 → 单文件 `plugin.mdl`

### 10.3 生态复用（npm 包）

- **插件就是合法 Node 项目**，npm 依赖是标准手段：`npm install` 产出 node_modules，`mdl build` 按 `package-lock.json` 快照把依赖拷入容器（可复现），不手动管理
- 纯 ES module JS 包可拷入容器（lodash-es、axios、zod 等），模块原样保留
- **不做自动 `require()`→ESM 重写**；CJS-only 包需作者在自有工具链中预转换（否则不支持）
- 提供轻量 Node API shim（`Buffer`、`path`、`events`）
- 依赖 Node 原生模块的包不可用，必须用宿主 API 替代（`import { storage, http } from "@metado/runtime"`）

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
| `setActive(plugin_id, caps)` / 能力开关 | 引擎产物激活面调整（`available`） |
| trace 订阅 | 流式轨迹开/关 |

- **引擎从不等待用户**：任何需人确认的动作（安装、resident-high、授权）= 宿主自己弹 UI → 事后调 RPC（grant/setLifecycle）

### 11.3 回调面（engine → host）

| 回调 | 语义 |
|---|---|
| `dispatch(name, params, requestId)` → host 返回 Value/Error | **capability message**：app 驻留服务收到引擎路由的值，执行后 resolve 插件 promise（v1 单次 Request-Response，逐次请求） |
| `notify(event)` | 事件流：`realmEvicted` / `quarantined(plugin_id)` / `pluginUpdated` / `pluginRemoved` |

### 11.4 权威划分

- **引擎 = 无主见执行者**：代码执行、权限裁决、生命周期、签名验签、沙箱、内置能力机制（模块在引擎内自给自足）
- **宿主 = 决策与管理**：信任决策（安装/削减/升级确认）、管理配置（白名单/配额/密钥供给/能力开关）、app 驻留服务（经 capability message）、观测消费
- 领域规则 v1 为引擎内配置驱动（setDomainConfig），不做宿主实时回调；将来需要宿主实时的走 `dispatch` 通道（§8.4）

### 11.5 值、错误、信任线

- 全链路同一值模型（§4.4）；RPC 序列化 JSON 兼容 + Bytes → `Uint8Array`
- `ExecutionError { entry, kind, message }` 原样跨 RPC 回宿主
- 插件签名信任不依赖 IPC（验签在引擎内、载荷进引擎前完成），宿主只管理"信不信该 signer"（§6）

## 12. 开发环境（CLI）

```
mdl build <plugin-dir> [--key <sign-key>]     # 组装模块树容器 + 签名
mdl run     <plugin.mdl> [--grant http.get]    # 运行引擎实例，可配置 grants 模拟生产
mdl watch                                      # 监听源文件 → 增量重建容器 → 热重载 → 自动 rerun
mdl test                                       # 插件测试（宿主 API 契约测试）
mdl trace                                      # 执行轨迹观测
mdl sign     <plugin.mdl> <key>                # 单独签名/验签
```

- **热重载**：复用热引擎，重载变更模块/入口，秒级反馈循环
- **轨迹观测（Trace）**：入口调用起止 / 宿主 API 调用参数与结果 / 每次权限裁决 requested vs granted / 值流转；设计为可复用观测 API，供未来交互式调试器挂接
- **行为对齐**：CLI 与生产共用同一引擎核心；验签、权限解析、沙箱语义零差异

## 13. 引擎 API 面（Rust，示意）—— 引擎内部 / 构建集成面

> 本 Rust API 是**引擎进程内部**与**宿主开发者构建期**使用的面（CLI / 测试 / 引擎 daemon 主程序 / 定制能力编译）；运行期的**宿主**只面对 §11 的 IPC 协议，不直接调这套 API。

```rust
let mut engine = Engine::new();
engine.register_capability(Http::default());        // / storage / file / time / log / crypto
engine.define_permission_set("standard", ["http.get", "log.info"]);

let plugin = engine.load_plugin("plugin.mdl").await?;   // 验签闸门 → 解析 → 静态检查 → 权限注入
let signer = plugin.signer_id();                        // 稳定发布者身份（公钥指纹）
let mode   = plugin.lifecycle();                        // 读 effective 常驻模式（宿主经 RPC 设）
engine.grant(&plugin, ["http.get.api.example"]);        // 构建期/CLI 直接授予（生产宿主经 RPC grant）

let out = plugin.invoke("onMessage", value).await?;
// Result<Value, ExecutionError>
```

更新验签：引擎按 plugin_id 保存首次安装的 `signer_pubkey` 指纹，后续同 plugin_id 的新 bundle 验签比对（§6.2-3）。

## 14. 范围分解与阶段

本设计由多个子系统组成，实现按阶段推进。另有**开放研究项**（先于对应阶段收敛、锁定）：

- **`@metado/runtime` 导出清单**：按 §8.5 API 风格原则逐一对照 Web/Node 约定，确定 http/storage/vfs/file/time/log/crypto/custom 的初版导出形状；锁定前不进入阶段 2 实施
- **boa 对 Web 类型/约定的支持面核验**（URL/Blob/TextEncoder/fetch 语义在 boa 下的现实缺口），反向约束导出形状选择
- **boa 的 WebAssembly 支持面**（实现 `metado-cap-wasm` 前核验）：决定复用标准 `WebAssembly` 全局还是引擎自托管 wasm 运行器

1. **引擎核心（metado-engine）**：签名验签、容器/manifest 解析（非自定义语言语法）、**入口/模块注册**、权限解析器、**值模型**、错误模型，以及**开放的能力框架（`#[capability]` 宏 + `CapabilitySet` trait、构建期注册）**（无 JS 执行）
2. **JS 执行器**：boa 桥接、**Node 风格模块解析**（exports field / node_modules 逐级）、模块系统挂载（容器文件树）、**`@metado/runtime` 虚拟内置模块映射**、值互转、事件循环集成、**执行预算**（interrupt/递归/栈限制）
3. **内置能力集（metado-cap-*）**：能力框架落地 + http/storage/vfs/file/time/log/crypto（含 signer 命名空间），独立 crate/feature
4. **CLI 工具（mdl）**：build（容器组装 + 签名）/ run / watch / test / trace，引擎实例 + 可配置 grants，npm 拷入
5. **引擎进程 + IPC**：transport 抽象、管理方法面、capability message 回调面、事件流，平台实现（Android/Win/Linux）
6. **示例与契约测试**：宿主开发者定制能力示例（构建期扩展性验证）、行为对齐验证、CLI/生产对比测试
7. **（推迟）Node 运行/测试包**：`@metado/runtime`（引擎侧已按规格供给）+ `metado-node`（napi 完整等价后端）、`@metado/testing` 测试架势

## 15. 明确推迟项

- 交互式单步调试器（trace API 预留扩展点）
- **Node 运行/测试包（推后）**：`@metado/runtime` 的 Node 侧真实供给 + `metado-node`（napi-rs 链 `metado-core` 的完整等价后端，真实权限裁决同源 Rust）、`@metado/testing`。**v1 不做**（纯 JS fallback 也不做）：需每平台原生构建矩阵 + V8/boa 语义对齐（由阶段 6 契约测试兜底）；但「能力 API = `@metado/runtime` 模块统一规格」的形态 v1 即遵循，引擎侧先行供给
- **TypeScript**：v1 源码 = 纯 ESM JS；`.ts` 编译暂不加入 `mdl build`（需先定编译器选型 swc-rs/esbuild、sourcemap 恢复 .ts 行号，并修订"零转换"承诺，契机再启）
- **WASM 计算内核（`metado-cap-wasm`）**：能力契约已定（§4.5），**v1 不实现**——wasmi 桥接、容器字节加载、插件 JS 实例化调用、fuel 计量、值进出无句柄，性能成为刚需时切 wasmtime
- 跨插件调用
- 零拷贝共享 buffer（`Bytes` 优化）
- 压缩（容器初版可不压缩，体积成为问题时再启用，分节内透传）
- 密钥轮换（同签更新缺失时的迁移路径，需升级签名 scheme 时再设计）
- 句柄式宿主对象跨 WASM 沙箱（永久拒绝）