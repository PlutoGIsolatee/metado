# Metado 设计文档

日期：2026-09-18
状态：待评审

## 1. 概述

Metado 是一个面向**跨平台插件开发**的 DSL 引擎项目（Rust，支持 Android / Windows / Linux）。插件以脚本形式交付，由引擎加载执行。插件主体用 JavaScript 编写，计算内核可用 WebAssembly。Metado 的核心价值是**细粒度权限治理层**：脚本级最小权限、用户可设置的授予、领域相关的极细分权限，以及权限集机制。

一句话定位：**一个能装进移动应用的小型、纯洁、可静态验证、带细粒度权限治理的插件容器。**

## 2. 目标与非目标

### 目标

- 跨平台（Android / Windows / Linux）嵌入能力，引擎为独立进程/服务
- 插件 = 脚本（JS 为主），可混合 WASM 计算内核，且两者高效进程内集成
- 细粒度权限治理作为一等公民：请求 / 授予 / 执行三层语义
- 接口约束（类型契约）加载期静态校验
- 单文件发布（Rollup 模式），多文件源码
- CLI 开发环境与生产环境行为对齐（单一引擎核心），支持快速迭代与热重载

### 非目标

- 通用编程语言能力（控制流、通用类型系统、标准库）—— 这些交给 JS/WASM
- 插件之间的相互调用（v1 不支持跨插件调用）
- 交互式单步调试器（v1 只做热重载 + 轨迹观测）
- 句柄式宿主对象跨入 WASM 沙箱（永久不变量）
- 管道/数据流模型（已明确砍掉，插件 = 命名单元集合）

## 3. 架构总览

```
                     ┌──────────────────────────────────────────┐
                     │            引擎核心（唯一）                │
                     │   DSL 解析器 │ 单元模型 │ 类型系统           │
                     │   权限解析器 │ 执行调度 │ 轨迹观测(Trace)    │
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
- **宿主 API 抽象**（`HostApi` trait）：CLI 提供 std 宿主（可配置 mock），生产宿主实现真实后端；引擎语义不变
- **自定义能力**：应用定制能力以类型化 capability message 路由到应用进程

## 4. 核心概念

### 4.1 单元（Unit）

插件 = 一组**命名单元**（替代早期的 "pipe" 概念，无自动串联）。

每个单元 = 名字 + 类型契约 + 实现 + 可见性。宿主按名调用。

```
@export @js[name="onMessage", type=Json->Json]:
  function main(input) {
    return { ... };
  }
```

- `@js` / `@wasm`：实现语言
- `name`：单元名，插件内唯一
- `type=In->Out`：接口契约，下文 4.2
- `@export` / 默认私有：可见性，下文 4.3

完整语法与解析规则作为实现细节，由配套的 DSL 文法文档（后续编写）定义。

### 4.2 类型契约（接口约束）

- `type=x->y` 中的类型名为**引擎级统一类型标识**，均指向引擎的 Rust 宿主类型
- 核心类型：`String / Number / Bool / Bytes / Json / List<T> / Any`
- 宿主可注册自定义类型（名字 + serde 实现）
- **连接校验：严格相等**。调用方期望类型与单元声明类型严格一致，只开放 `Any` 作为显式逃生口
- 运行时校验单元输出与声明类型一致，不一致即失败

### 4.3 可见性

- 单元默认私有，仅本插件内其他单元可按名调用
- `@export` 标记的单元为插件公开 API，宿主可通过 IPC 直接调用
- 跨插件调用 v1 不支持

### 4.4 WASM 值边界（架构不变量）

> **句柄/对象身份永不跨入 WASM 沙箱。** WASM 单元永远是纯函数式值进出。

- WASM 单元只能接收/返回普通值（经线性内存搬运），无句柄、无对象身份、无生命周期管理
- 好处：离线可测、沙箱最紧、唯一资源管控 = fuel 计量 + 输入大小
- 需要长期状态的场景放 JS 层；WASM 只管纯计算内核
- wasmi 作为 v1 运行时（纯 Rust 解释器，内置 fuel 计量，跨平台零障碍）。API 镜像 wasmtime，未来可替换

## 5. 权限模型（核心）

三层要素：

```
① 请求（DSL 声明）                ② 授予（宿主/用户运行时决定）             ③ 执行（每次调用检查）
permission:                        engine.grant(plugin_id, { ... })          host_api 调用 →
 [http.get.api.example,             // granted ⊆ requested                     resolver.check(
  storage.read.profile]              // 用户可削减，领域规则叠加                    requested, granted,
                                    // engine.define_permission_set(...)          domain_rules) → pass/deny
```

### 5.1 权限语法（点分式）

- 细粒度点分式：`http.get`、`http.get.api.example`、`storage.read.profile`、`log.info`
- 声明位置：脚本头部 `permission:` 字段

### 5.2 三层语义

- **最小权限**：加载时注入的 API 视图 = `requested ∩ available`；未请求的函数绑定不存在（访问即快速失败）。静态可审计，默认拒绝
- **用户可设**：`granted ⊆ requested`，运行时由宿主应用决定实际授予范围；每次宿主 API 调用经运行时执行层复核
- **领域规则**：宿主可注册领域级 resolver 叠加（如"http 仅限本应用白名单域名"、"调用次数超限自动降级"）

### 5.3 权限集

- 宿主应用定义命名权限集，插件引用：

```rust
engine.define_permission_set("standard", ["http.get", "log.info"]);
engine.define_permission_set("finance",  ["http.get.api.bank", "storage.read", "crypto.sign"]);
```

- 插件 DSL 头部：`permission-set: [standard, finance]`
- 插件内联定义权限集仅用于内部复用
- 加载期检查：插件引用的权限集必须已由宿主定义，否则加载失败

## 6. 宿主能力与自定义能力

### 6.1 内置能力集（capability set）

- 引擎进程自带丰富内置能力，按能力集组织，宿主按需组合构建
- Rust 侧用宏/trait 声明式定义：`#[capability]` / `impl CapabilitySet`，定义能力名（对应权限名）与实现
- 初版内置：`http / storage / file / time / log / crypto`

### 6.2 自定义能力（capability message）

- 应用定制能力（UI、业务服务）以**类型化能力消息**提供，不走函数调用/句柄
- 插件调用：`const r = await metado.custom("ui.alert", {title, body})`
- 引擎把值路由给应用订阅的 capability channel；应用执行后可选返回一个值 → resolve 为 promise，插件 `await` 继续
- 边界纪律：跨进程永远传值，不传句柄；每类 message 有 schema，可版本化

### 6.3 JS 侧 API 访问面（访问风格收敛）

- **内置能力**：授权集内的能力函数作为**模块绑定**自动注入每个单元作用域（如 `http.get(...)` 直接可用），无需 import 样板
- **引擎全局对象 `metado`**：唯一的保留全局，承载引擎运行设施（`metado.log`、`metado.abort`、权限自省）与自定义能力派发（`metado.custom`）
- 未授权的绑定不存在 → 访问即快速失败；两种访问风格都经过同一个权限解析器

### 6.4 HostApi 抽象

`HostApi` trait 是引擎与宿主能力的唯一接口。CLI 的 std 宿主与生产宿主都实现它，保证行为一致。

## 7. 执行与错误处理

### 7.1 执行

- JS 单元：`async function main(input)`，boa 驱动，引擎单一事件循环推进 job queue
- WASM 单元：同步式调用（值经线性内存进出），执行使用 fuel 计量，不干扰事件循环
- 宿主按名调用 exported 单元：`plugin.invoke(unit_name, input)`
- 值边界统一：值跨 JS/WASM/宿主任意边界使用同一套引擎级值表示（引擎值 model）

### 7.2 错误处理

**一概短路中止，无错误作为值的传播。**

- 加载/静态期错误（语法、权限声明非法、类型不衔接、引用未授权 API、权限集未定义）在 `load_plugin` 时 fail-fast
- 运行期错误 → 结构化 `PipelineError { unit, name, kind: Runtime|Permission|Sandbox, message }`，由宿主决定处置
- 沙箱/环境错误（fuel 耗尽、WASM trap、序列化失败）永远中止
- 插件作者需要"失败即结果"的场景，在 JS 层用 try/catch 自行消化

## 8. 打包与构建

### 8.1 源码（多文件）

```
myplugin/
  mdl.toml              # name / version；units 目录
  units/
    on_message.mdl      # @export @js[name="onMessage", type=Json->Json]: ...
    transform.mdl       # @wasm[name="transform", type=Bytes->Bytes, exports=[transform]]
```

### 8.2 分发（单文件）

- `mdl build` 内联所有单元 + WASM（base64）成单个纯文本 `plugin.mdl`
- WASM 以 base64 内联进同一文本容器（初版；如未来出现大负载瓶颈再升级二进制容器）

### 8.3 生态复用（npm 包）

- 纯 ES module JS 包可打包进插件（lodash-es、axios、zod 等）
- 打包工具对 `require()` 做 ESM 重写；提供轻量 Node API shim（`Buffer`、`path`、`events`）
- 依赖 Node 原生模块的包不可用，必须用宿主 API 替代（`metado.storage` / `metado.http`）

## 9. 进程模型与通信

- 引擎为独立进程/服务：故障隔离（插件崩溃不拖垮宿主）、宿主语言解耦、沙箱升级路径
- IPC transport 抽象层各平台实现：Android bound service、Win/Linux Unix domain socket
- 协议线格式：**JSON-RPC 2.0**（v1 默认；值传递原则下简单可调试，若体积成为问题再引入二进制编码）
- 协议边界均为值传递；通用能力内置（零 IPC），自定义能力走 capability message

## 10. 开发环境（CLI）

```
mdl run  <plugin.mdl> [--grant http.get]   # std 宿主执行，可配置 grants 模拟生产
mdl watch                                   # 监听源文件 → 增量重建 → 热重载 → 自动 rerun
mdl test                                    # 单元/集成测试（宿主 API 契约测试）
mdl trace                                   # 执行轨迹观测
```

- **热重载**：复用热引擎，重载变更单元，秒级反馈循环
- **轨迹观测（Trace）**：单元起止 / 宿主 API 调用参数与结果 / 每次权限裁决 requested vs granted / 值流转；设计为可复用观测 API，供未来交互式调试器挂接
- **行为对齐**：CLI 与生产共用同一引擎核心；权限解析、类型契约、沙箱语义零差异

## 11. 引擎 API 面（Rust，示意）

```rust
let mut engine = Engine::new();
engine.register_capability(Http::default());        // / storage / file / time / log / crypto
engine.register_type::<MyType>("MyType");            // 可选：注册自定义宿主类型
engine.define_permission_set("standard", ["http.get", "log.info"]);

let plugin = engine.load_plugin("plugin.mdl").await?;   // 静态检查全部在此暴露
engine.grant(&plugin, ["http.get.api.example"]);        // 宿主授予（可被用户削减）

let out = plugin.invoke("onMessage", json!({...})).await?;
// Result<Value, PipelineError>
```

## 12. 范围分解与阶段

本设计由多个子系统组成，实现按阶段推进：

1. **引擎核心**：DSL 解析、单元模型、类型系统、权限解析器、值表示、错误模型（无 JS/WASM 执行）
2. **JS 执行器**：boa 桥接、模块注入、值互转、事件循环集成
3. **WASM 执行器**：wasmi 桥接、base64 加载、import 函数、fuel 计量
4. **内置能力集**：macro/能力框架 + http/storage/file/time/log/crypto
5. **CI 工具（mdl）**：build / run / watch / test / trace，std 宿主，npm 打包
6. **引擎进程 + IPC**：transport 抽象、capability message 路由、平台实现（Android/Win/Linux）
7. **示例与契约测试**：行为对齐验证、CLI/生产对比测试

## 13. 明确推迟项

- 交互式单步调试器（trace API 预留扩展点）
- wasmtime（wasmi 镜像其 API，性能成为刚需时切换）
- 跨插件调用
- 零拷贝共享 buffer（`Bytes` 优化）
- 二进制插件容器（wasm base64 内联体积成为问题时）
- 句柄式宿主对象跨 WASM 沙箱（永久拒绝）

## 14. 配套文档

- DSL 文法细节（注解语法、权限表达式、内联权限集语法）—— 由配套**文法文档**定义，作为阶段 1 实施依据
- IPC 线格式已定：JSON-RPC 2.0（见第 9 节）