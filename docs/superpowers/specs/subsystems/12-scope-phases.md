# Metado 范围分解与阶段

## 14. 范围分解与阶段（修订）

本设计由多个子系统组成，实现按阶段推进。另有**开放研究项**（先于对应阶段收敛、锁定）：

- **`@metado/runtime` 导出清单**：按 §8.5 API 风格原则逐一对照 Web/Node 约定，确定 http/storage/vfs/file/time/log/crypto/custom 的初版导出形状；锁定前不进入阶段 2 实施
- **boa 对 Web 类型/约定的支持面核验**（URL/Blob/TextEncoder/fetch 语义在 boa 下的现实缺口），反向约束导出形状选择
- **`metado-cap-wasm` 实现前核验**：boa 的 WebAssembly 支持面（复用标准面 vs 引擎自托管运行器，§4.5）；**WASM esm-integration 草案语义细节**（named exports / global 解开 / default 语义，参照 Vite 8.1 与 Node 实现收敛）；Node 侧 `.wasm` import 的 parity 路径（Node 实验支持 vs 作者经 Vite，为 `@metado/runtime` Node 供给决策）；`instantiateStreaming` 是否纳入双面

1. **引擎核心（metado-engine）**：签名验签、容器/manifest 解析（非自定义语言语法）、**入口/模块注册**、权限解析器、**值模型**、错误模型，以及**开放的能力框架（`#[capability]` 宏 + `CapabilitySet` trait、构建期注册）**、**状态持久化（engine/state.json、单写者、原子替换、文件锁）**（无 JS 执行）
2. **JS 执行器**：boa 桥接、**Node 风格模块解析**（exports field / node_modules 逐级）、模块系统挂载（容器文件树）、**`@metado/runtime` 虚拟内置模块映射**、值互转、事件循环集成、**执行预算**（interrupt/递归/栈限制）
3. **内置能力集（metado-cap-*）**：能力框架落地 + http/storage/vfs/file/time/log/crypto（含 signer 命名空间），独立 crate/feature
4. **CLI 工具（mdl）**：build/sign/verify、run/watch（开发密钥自动签名 + 热重载）、test、env、trace（§12），引擎 in-process 宿主 + 可配置 grants / --ask 授权模拟，npm 拷入
5. **引擎进程 + IPC**：transport 抽象、管理方法面、capability message 回调面、事件流，平台实现（Android/Win/Linux）
6. **示例与契约测试**：宿主开发者定制能力示例（构建期扩展性验证）、行为对齐验证、CLI/生产对比测试
7. **（推迟）Node 运行/测试包**：`@metado/runtime`（引擎侧已按规格供给）+ `metado-node`（napi 完整等价后端）、`@metado/testing` 测试架势