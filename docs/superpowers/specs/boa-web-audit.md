# boa Web API Support Audit

日期：2026-09-19

## API 支持矩阵

| API | boa Status | Gap | Mitigation |
|-----|-----------|-----|------------|
| URL / URLSearchParams | ✅ native | - | - |
| TextEncoder / TextDecoder | ✅ native | - | - |
| Blob | ❌ missing | - | 能力层自行实现 (storage/file 用 Bytes) |
| ReadableStream | ❌ missing | - | v1 不做流式, 可后置 |
| WritableStream | ❌ missing | - | v1 不做流式 |
| fetch / Request / Response / Headers | ❌ missing | - | 能力层自行实现 (reqwest 后端), 不依赖 boa fetch |
| WebCrypto (subtle) | partial | - | 仅 hash/hmac, 不需 subtle |
| queueMicrotask | ✅ native | - | - |
| globalThis | ✅ native | - | - |
| Event / CustomEvent | ❌ missing | - | 能力层不依赖 DOM 事件 |
| ArrayBuffer / Uint8Array | ✅ native | - | - |
| SharedArrayBuffer | ❌ missing | - | v1 不需要 |
| structuredClone | ❌ missing | - | Value 模型序列化替代 |
| console | ✅ native | - | - |
| setTimeout / setInterval | ✅ native | - | - |
| Promise | ✅ native | - | - |
| Symbol | ✅ native | - | - |
| Proxy | ✅ native | - | - |
| Reflect | ✅ native | - | - |
| WeakRef / FinalizationRegistry | ❌ missing | - | v1 不需要 |

## 结论

1. **核心 Web 类型已就绪**: URL, TextEncoder, TextDecoder, ArrayBuffer, Uint8Array
2. **fetch 需能力层实现**: 不依赖 boa 内置 fetch, metado-cap-http 用 reqwest 实现
3. **Blob 需自行实现**: storage/file 能力用 Bytes (Uint8Array) 替代
4. **流式 API v1 不做**: ReadableStream/WritableStream 留到 v2
5. **WebCrypto 仅需 hash/hmac**: 不需完整 subtle 接口

## 反向约束

- `http` 导出不返回 `Response` 对象, 返回 `{ status, headers, body }` 简化结构
- `storage` 导出用 `Uint8Array`, 不用 `Blob`
- `crypto` 导出仅 randomBytes/sha256/hmac, 不暴露 subtle
- `file` 导出返回 `Uint8Array`, 不用 `ReadableStream`
