# Metado 明确推迟项

## 15. 明确推迟项

- 交互式单步调试器（trace API 预留扩展点）
- **Node 运行/测试包（推后）**：`@metado/runtime` 的 Node 侧真实供给 + `metado-node`（napi-rs 链 `metado-core` 的完整等价后端，真实权限裁决同源 Rust）、`@metado/testing`。**v1 不做**（纯 JS fallback 也不做）：需每平台原生构建矩阵 + V8/boa 语义对齐（由阶段 6 契约测试兜底）；但「能力 API = `@metado/runtime` 模块统一规格」的形态 v1 即遵循，引擎侧先行供给
- **TypeScript**：v1 源码 = 纯 ESM JS；`.ts` 编译暂不加入 `mdl build`（需先定编译器选型 swc-rs/esbuild、sourcemap 恢复 .ts 行号，并修订"零转换"承诺，契机再启）
- **WASM 计算内核（`metado-cap-wasm`，内置 API）**：能力契约已定（§4.5），**v1 不实现**——标准 `WebAssembly` 命名空间 + 直接 `.wasm` 导入（esm-integration 草案）双面，同一实现内核、同一执行预算；**WASI 永久排除**（无 OS 接口面，与无 syscall / 值进出无句柄一致）；wasmtime 留作性能刚需时切换
- 跨插件调用
- 零拷贝共享 buffer（`Bytes` 优化）
- 压缩（容器初版可不压缩，体积成为问题时再启用，分节内透传）
- 密钥轮换（同签更新缺失时的迁移路径，需升级签名 scheme 时再设计）
- 句柄式宿主对象跨 WASM 计算内核（永久拒绝）
- **流式宿主 API**：大载荷增量传输（分块 slice + 背压 + 取消 + 途中错误/生命周期）；v1 以整值 + 容量上限（§8.5、§5.2）替代，进度/推送经 notify；待零拷贝共享 buffer 与 boa 的 ReadableStream 支持面核验（§14）后以显式 opt-in 能力形状落地