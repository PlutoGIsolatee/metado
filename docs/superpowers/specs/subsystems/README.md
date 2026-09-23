# Metado 设计文档 - 子系统索引

> 基于 `2026-09-18-metado-design.md`（rev 11）拆分的子系统文档集合

---

## 子系统列表

| 编号 | 子系统 | 文件 | 说明 |
|------|--------|------|------|
| 01 | 概述与目标 | [01-overview.md](01-overview.md) | 概述、目标与非目标 |
| 02 | 架构总览 | [02-architecture.md](02-architecture.md) | 整体架构图、引擎进程、IPC、状态持久化 |
| 03 | 核心概念 | [03-core-concepts.md](03-core-concepts.md) | 插件形态、入口、内部模块、值模型、WASM内核 |
| 04 | 权限模型 | [04-permissions.md](04-permissions.md) | 三层权限、语法、权限集 |
| 05 | 签名与信任 | [05-signature-trust.md](05-signature-trust.md) | Ed25519签名、同签更新、同签互通 |
| 06 | 插件生命周期 | [06-lifecycle.md](06-lifecycle.md) | 契约、Realm生命、常驻模式、状态模型、状态机、卸载、并发 |
| 07 | 宿主能力与框架 | [07-host-capabilities.md](07-host-capabilities.md) | 能力框架、内置能力、自定义能力、JS API |
| 08 | 执行与错误处理 | [08-execution-errors.md](08-execution-errors.md) | 执行模型、错误处理、四故障域 |
| 09 | 打包与构建 | [09-packaging-build.md](09-packaging-build.md) | 源码结构、MDL分发、npm生态复用 |
| 10 | 引擎-宿主交互 | [10-engine-host-interaction.md](10-engine-host-interaction.md) | IPC、管理方法面、回调面、权威划分 |
| 11 | CLI 开发环境 | [11-cli.md](11-cli.md) | CLI定位、命令、watch、授权模拟、env、trace |
| 11b | 引擎 API | [11-engine-api.md](11-engine-api.md) | Rust API 面、Engine、PluginRuntime |
| 12 | 范围分解与阶段 | [12-scope-phases.md](12-scope-phases.md) | 7个阶段、开放研究项 |
| 13 | 明确推迟项 | [13-deferred.md](13-deferred.md) | v1不做项、推迟到后续版本 |
| 14 | TypeScript 支持 | [14-typescript-support.md](14-typescript-support.md) | v1不实现，v1.1+运行时编译+缓存、仅transpile |

---

## 快速导航

| 关注点 | 相关子系统 |
|--------|-----------|
| 权限系统 | 04-permissions, 06-lifecycle (§7) |
| 签名/信任 | 05-signature-trust |
| 生命周期 | 06-lifecycle |
| 权限模型 | 04-permissions, 06-lifecycle (§7.3) |
| 存储/状态 | 06-lifecycle (§7.3), 10-engine-host-interaction (§11.2) |
| CLI/开发 | 11-cli |
| 架构/部署 | 02-architecture, 09-packaging-build |
| IPC/交互 | 10-engine-host-interaction |
| 扩展/能力 | 07-host-capabilities |
| 错误处理 | 08-execution-errors |
| TypeScript 支持 | 14-typescript-support |
| 实现计划 | 12-scope-phases |

---

## 关键修订记录（相对于 rev 10）

| 变更 | 说明 |
|------|------|
| 状态持久化 | 新增 `engine/state.json`，单写者、原子替换、文件锁、启动自洽恢复 |
| 术语统一 | `store`→`state`、`governance`→`engine`、`plugin-data`→`plugins/`/`shared/` |
| 生命周期 | `pending` 取消，`granted` 空可运行，三态模型 |
| 同签更新 | 移除首装指纹锚，改为当前存储本体重算比对 |
| `active` | 运行时不可变、启动配置、不进 state |
| `setActive` | 从管理面移除，改为启动配置 |
| `setState/getState` | 新增管理面 RPC |
| 可重算数据 | 一律不持久化（signer_id/version/requested/entries 从 .mdl 重算） |
| 同签判据 | 改为当前存储本体比对，移除首装指纹锚 |
| TypeScript | v1 不支持，v1.1+ 运行时编译+缓存、仅 transpile、无增量、无类型检查 |