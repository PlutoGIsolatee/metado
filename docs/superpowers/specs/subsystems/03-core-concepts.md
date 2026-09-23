# Metado 核心概念

## 4.1 插件形态

插件 = **标准 ESM JavaScript 代码 + 结构化 manifest 元数据**，源码侧是一个**合法 Node 项目**（除 API），交付为一个签名的二进制单文件容器：

```
plugin.mdl =
  签名信封（signer_pubkey, signature, algorithm）      §6
  ZIP 模块树容器（manifest + src + node_modules + wasm）  §10
```

## 4.2 入口（Entries）

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

## 4.4 值模型（Value Model）

- 引擎统一值表示：`Null / Bool / Number / String / Bytes / List / Json-Map`（JSON 兼容 + Bytes）—— 值跨 **JS / 宿主 / IPC** 三条边界同一表示；WASM 内核不直接承载值模型，经 JS 桥接标量/线性内存（§4.5）
- 入口只传值：宿主 invoke 传 Value，JS 侧映射标准 JS 值；JS 侧无法序列化的对象（函数/句柄/循环引用）不跨边界
- **无 manifest 级类型声明**：接口由 JS 语言自身与宿主侧期望承担；"接口是什么"是文档/测试层的事（`mdl test`），引擎不掺和
- Bytes 为明确值类型：跨 IPC 与 `Uint8Array`/`ArrayBuffer` 对应
- WASM 内核的 IO 契约 = **端口带**（JS 标量 / 线性内存 Bytes），属 JS 侧调用方约定，不由 manifest 声明（§4.5）

## 4.5 WASM 计算内核（内置 API；**实现推迟 v1 后**）

> **WASM = 内置 API（`metado-cap-wasm`）**，与 http/storage 同待遇；它住在插件 realm 内，由插件 JS 编排，跨 `.mdl` 与 npm 交付的资源格式携带（wasm/ 原样字节）。**值进出无句柄/对象身份**——纯函数式计算承载面，状态留在 JS 侧。

- **JS 双面形状（同一实现内核）**：
  - **标准 `WebAssembly` 命名空间（Node 对齐）**：`Module / Instance / Memory / Table / Global` + 错误类型 `CompileError / LinkError / RuntimeError` + `compile / validate / instantiate`——wasm-bindgen / wit-bindgen 等**自动胶水**路径（胶水自读字节 + `instantiate(bytes, imports)`）
  - **直接 `.wasm` 模块导入（WASM esm-integration 草案）**：`import { add } from "./transform.wasm"`，named exports = wasm exports——对齐 Vite 8.1 作者工作流 / 浏览器 / Node 未来（草案语义细节为 §14 锁定项）
- **字节获取**：胶水路径经 vfs/file 资源读（`file.read("wasm/transform.wasm") → Bytes`，Node 等价职 `fs.readFile`）；手工路径直接 `import` .wasm（模块加载器按模块类型解析）
- **权限 = 可用性门**：无独立权限名；`WebAssembly` 命名空间可导入 / `.wasm` 模块类型可用 = 已授（`available`），调用不逐次裁决
- **执行预算统一**：内核执行计入**同一执行预算**（JS interrupt + 内核 fuel 一个资源视图），防失控、决定论、离线可测
- **错误映射（§9.2）**：WASM trap = `PluginError`（JS 可捕获，realm 存活）；fuel 耗尽 = `Fault`（引擎判 realm，§7）
- **WASI 永久排除**（§15）：无 OS 接口面，与无 syscall / 值进出无句柄一致
- **v1 纯 JS**：引擎无 WASM 运行器；较重计算走宿主定制能力 / capability message，JS 防失控用执行预算
- **v1 后实现**：boa 自带 WebAssembly 则复用标准面；否则引擎自托管运行器（wasmi 桥接，纯 Rust 解释器、内置 fuel、跨平台零障碍）；性能成为刚需时切 wasmtime
- 引擎侧运行器与 Node 侧语义对齐为 §14 核验项