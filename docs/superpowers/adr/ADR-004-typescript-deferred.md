# ADR-004：TypeScript 推迟（v1 不做）

状态：已接受（推迟项，v1.1+ 实现）
日期：2026-09-24

背景：v1 是否支持 TypeScript 插件源码。

选项：
  - A：构建期预编译（swc/esbuild，`.ts` → `.js` + sourcemap 打包进 `.mdl`）。
    优点：运行时无编译开销。缺点：破坏"零转换"承诺，分发物与源码不一致。
  - B：运行时即时编译 + 缓存（纯 `.ts` 打包，实例化前编译）。
    优点：零转换承诺完整保持。缺点：运行时需 swc，冷启动有编译开销；swc 无原生增量编译，自建成本高。
  - C：v1 不做。
    优点：v1 核心（真实执行链路）不受阻。缺点：TS 用户等待。

决定：选 C；v1.1+ 按 B 落地，且仅 transpile、无增量编译、无类型检查（类型检查仅 `mdl test`/CI 可选 `tsc --noEmit`）。

后果：`mdl build` 纯打包签名；v1 源码 = 纯 ESM JS；完整设计见 `specs/subsystems/14-typescript-support.md`。
