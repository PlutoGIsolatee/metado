# @metado/runtime Export Spec v1

日期：2026-09-19
状态：锁定

> 本规格锁定 v1 `@metado/runtime` 导出清单，作为引擎侧虚拟模块实现的验收标准。

## 导出原则

1. **Web Platform 优先**：标准形状直接镜像（fetch 风格 http、WebCrypto 风格 crypto）
2. **Node whitelist shim 保持 Node 形状**：Buffer/path/events，为 npm 互操作
3. **无标准可依处局部自研最小面**：storage 的 signer 命名空间、vfs、time/log、capability dispatch
4. **只镜像形状，不镜像权限语义**：每次调用仍过 realm 授权视图
5. **导出存在性 = 静态面（`available`），调用放行 = 动态面（`granted`）**

## 导出清单

### http

| Export | Shape | Web Reference | Notes |
|--------|-------|---------------|-------|
| `get(input: RequestInfo, init?: RequestInit): Promise<Response>` | fetch 风格 | Fetch API | 走 realm 授权视图 |
| `post(input: RequestInfo, init?: RequestInit): Promise<Response>` | fetch 风格 | Fetch API | |

权限名: `http.get`, `http.post`, `http.get.api.<domain>`

### storage

| Export | Shape | Notes |
|--------|-------|-------|
| `read(key: string): Promise<Uint8Array>` | 自研 | signer namespace via `storage.<signer>.read` |
| `write(key: string, data: Uint8Array): Promise<void>` | 自研 | signer namespace via `storage.<signer>.write` |

Keyspace:
- 私有: `storage:<plugin_id>:<key>`
- 共享: `storage:<signer_id>:<key>` (需 `storage.<signer>.write` 权限)

### file

| Export | Shape | Notes |
|--------|-------|-------|
| `read(path: string): Promise<Uint8Array>` | 自研 | read-only vfs |
| `stat(path: string): Promise<{size: number, mtime: number}>` | 自研 | |

### time

| Export | Shape | Notes |
|--------|-------|-------|
| `now(): number` | Date.now() | epoch ms |
| `sleep(ms: number): Promise<void>` | 标准 | |

### log

| Export | Shape | Notes |
|--------|-------|-------|
| `info(...args: any[]): void` | console 风格 | |
| `warn(...args: any[]): void` | console 风格 | |
| `error(...args: any[]): void` | console 风格 | |
| `debug(...args: any[]): void` | console 风格 | |

### crypto

| Export | Shape | Notes |
|--------|-------|-------|
| `randomBytes(n: number): Uint8Array` | WebCrypto 风格 | |
| `sha256(data: Uint8Array): Promise<Uint8Array>` | WebCrypto 风格 | |
| `hmac(key: Uint8Array, data: Uint8Array): Promise<Uint8Array>` | WebCrypto 风格 | |

### custom

| Export | Shape | Notes |
|--------|-------|-------|
| `dispatch(name: string, params: JsonValue): Promise<JsonValue>` | 自研 | capability message, v1 单次 Request-Response |

### Buffer/path/events (shim)

| Export | Shape | Notes |
|--------|-------|-------|
| `Buffer` | Node Buffer | re-export for npm interop |
| `path` | Node path | re-export for npm interop |
| `events` | Node events | re-export for npm interop |

## 错误类型

| Class | extends | Fields |
|-------|---------|--------|
| `ExecutionError` | Error | `entry: string`, `kind: string`, `message: string` |
| `PermissionDenied` | ExecutionError | `permission: string` |

## 引擎实现映射

| Export | Engine Implementation | Crate | Notes |
|--------|-----------------------|-------|-------|
| http.get() | reqwest 后端 | metado-cap-http | 走 realm 授权 + domain rules |
| http.post() | reqwest 后端 | metado-cap-http | |
| storage.read() | filesystem 后端 | metado-cap-storage | keyspace: `storage:<plugin_id>:<key>` |
| storage.write() | filesystem 后端 | metado-cap-storage | signer namespace via `<signer>` 绑定 |
| file.read() | read-only mount | metado-cap-file | 容器内 wasm/ 目录 |
| file.stat() | read-only mount | metado-cap-file | |
| time.now() | SystemTime | metado-cap-time | epoch ms |
| time.sleep() | tokio::time::sleep | metado-cap-time | |
| log.info/warn/error/debug | eprintln! / tracing | metado-cap-log | |
| crypto.randomBytes() | rand::thread_rng | metado-cap-crypto | |
| crypto.sha256() | sha2 crate | metado-cap-crypto | |
| crypto.hmac() | hmac crate | metado-cap-crypto | |
| custom.dispatch() | IPC dispatch callback | metado-engine | v1 单次 Request-Response |
| Buffer | boa polyfill | metado-executor | 仅形状, 不承诺 Node 行为 |
| path | boa polyfill | metado-executor | |
| events | boa polyfill | metado-executor | EventEmitter 最小面 |

## 使用示例

```js
import { http, storage, log, Buffer } from "@metado/runtime";

export async function onMessage(input) {
  log.info("Received:", input);

  // HTTP (需 http.get 权限)
  const res = await http.get("https://api.example.com/data");
  const data = await res.json();

  // Storage (需 storage.write 权限)
  await storage.write("cache/data", new TextEncoder().encode(JSON.stringify(data)));

  // Buffer (shim, 无需权限)
  const buf = Buffer.from("hello");
  log.info("Buffer:", buf);

  return { ok: true, data };
}
```
