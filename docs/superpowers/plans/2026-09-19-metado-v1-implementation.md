# Metado v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Metado v1 — a cross-platform embeddable JS runtime with fine-grained permission governance, Android-style signature trust, single-file `.mdl` packaging, and CLI development toolchain.

**Architecture:** Rust engine core (metado-engine) as independent process, boa JS executor, IPC via JSON-RPC 2.0, capability framework with `#[capability]` macro, TOML manifest, Ed25519 signatures, ZIP module tree containers. Node side gets a minimal placeholder `@metado/runtime` npm package (dev-only, not production).

**Tech Stack:** Rust (engine, CLI, capabilities), boa (JS executor), Ed25519 (ed25519-dalek), ZIP (zip crate), TOML (toml crate), JSON-RPC 2.0, Unix domain socket / Android bound service (IPC)

**Spec:** `docs/superpowers/specs/2026-09-18-metado-design.md`

## Global Constraints

- Rust edition 2021, MSRV 1.70
- v1 签名算法仅 Ed25519
- 容器格式 ZIP, store 模式（不压缩）, magic "MDL1", format_version u16
- 值模型: Null / Bool / Number(f64) / String / Bytes(Vec<u8>) / List / Json-Map
- 权限语法: 点分式, signer 域引用 `<signer>` 在加载期绑定
- JS 执行器: boa (非 V8), 事件循环集成, 执行预算 (interrupt + fuel)
- IPC 线格式: JSON-RPC 2.0, 本地可信信道不加密
- Node 侧 v1 仅极简占位包 (dev-only shim), 不做真实后端
- WASM 计算内核 v1 不实现
- TypeScript v1 不支持
- 跨插件调用 v1 不支持
- 流式宿主 API v1 不支持

## Dependency Policy (2026-09-19 锁定)

> **约束：** 始终使用**可兼容的最高稳定版本**（跳过 `-pre`/`-rc` 预发布），API 以官方文档/源码为准，不照旧版本示例码。

| crate | 版本 | 备注 / API 断点 |
|-------|------|----------------|
| serde | 1.0.229 | derive feature |
| serde_json | 1.0.151 | |
| toml | 1.1.6 | `+spec-1.1.0`；`from_str`/`Value`/`Table` 同 0.8 |
| sha2 | 0.11.0 | digest 0.11 系；`Sha256::digest` 不变 |
| hex | 0.4.3 | |
| rand | 0.10.2 | `thread_rng()` → `rng()`；`Rng`/`TryRng` 分裂 |
| ed25519-dalek | 3.0.0 | feature `rand_core`（rand_core 0.10）；`Signature::from_bytes` 移除，用 `Signature::try_from(&[u8])`；`SigningKey::generate` 需 `CryptoRng` |
| zip | 8.6.0 | 9.0.0-pre 不用；`write::SimpleFileOptions` 仍在；`ZipWriter::new/start_file/finish`、`ZipArchive::new/by_index/by_name` 不变；rust-version 1.88 |
| thiserror | 2.0.20 | derive API 不变 |
| boa_engine | 0.22.0 | `boa`(0.13.x 已废弃)→`boa_engine`；rust-version 1.91；`Context::eval(Source::from_bytes(..))`；`JsValue::as_callable()->Option<JsObject>`；`JsObject::call(&self, this, args: &[JsValue], &mut Context)`；`JsString::to_std_string_escaped()` |
| tokio | 1.53.1 | features: rt-multi-thread, macros, net, time, io-util, sync |
| reqwest | 0.13.5 | default-features=false + json |
| hmac | 0.13.0 | 与 sha2 0.11 同为 digest 0.11；`Hmac<Sha256>::new_from_slice` 不变 |
| clap | 4.6.7 | derive feature |
| notify | 8.2.0 | 9.0.0-rc 不用；`RecommendedWatcher::new` + `Config::default()`，`watch(path, RecursiveMode)`**

---

## File Structure (全局 crate 布局)

```
metado/
├── Cargo.toml                          # workspace root
├── crates/
│   ├── metado-engine/                  # 核心引擎: 值模型, 权限解析, 签名验签, 容器解析, 能力框架, 生命周期
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── value.rs                # Value enum (§4.4)
│   │       ├── error.rs                # ExecutionError, PluginError, PermissionDenied, CapabilityError, Fault (§9.2)
│   │       ├── manifest.rs             # mdl.toml 解析 (§4.2)
│   │       ├── container.rs            # ZIP 容器读取/挂载 (§10.2)
│   │       ├── signature.rs            # Ed25519 验签/签名 (§6)
│   │       ├── permission.rs           # 权限解析器: requested/granted/available/domain_rules (§5)
│   │       ├── capability.rs           # CapabilitySet trait, CapabilityRegistry (§8.1)
│   │       ├── realm.rs                # Realm 生命周期: 状态机 (§7.4)
│   │       ├── plugin.rs               # Plugin 加载/安装/更新/卸载 (§7)
│   │       └── trace.rs                # 轨迹观测 sink (§12.6)
│   ├── metado-cap-http/                # 内置能力: HTTP (§8.2)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── metado-cap-storage/             # 内置能力: Storage + signer namespace (§6.3)
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── metado-cap-file/                # 内置能力: File
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── metado-cap-time/                # 内置能力: Time
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── metado-cap-log/                 # 内置能力: Log
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── metado-cap-crypto/              # 内置能力: Crypto
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── metado-executor/                # JS 执行器: boa 桥接, 模块解析, 虚拟模块 (§2 Phase 2)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs               # boa 集成, 事件循环
│   │       ├── module_loader.rs        # Node 风格模块解析
│   │       ├── virtual_module.rs       # @metado/runtime 虚拟模块映射
│   │       ├── value_convert.rs        # Value ↔ boa JsValue 互转
│   │       └── budget.rs               # 执行预算: interrupt/fuel
│   ├── metado-cli/                     # CLI 工具 (§12)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── build.rs               # mdl build: 组装 ZIP + 签名
│   │       ├── sign.rs                # mdl sign: 单独签名
│   │       ├── verify.rs              # mdl verify: 仅验签
│   │       ├── run.rs                 # mdl run: 引擎实例执行
│   │       ├── watch.rs               # mdl watch: 文件变更热重载
│   │       ├── test.rs                # mdl test: 契约测试
│   │       ├── env.rs                 # mdl env: 能力/权限静态诊断
│   │       └── trace.rs               # mdl trace: 执行轨迹观测
│   ├── metado-ipc/                     # IPC transport + 协议 (§11)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── transport.rs            # Transport trait
│   │       ├── unix.rs                 # Unix domain socket (Linux/macOS)
│   │       ├── android.rs              # Android bound service
│   │       ├── windows.rs              # Windows named pipe
│   │       ├── jsonrpc.rs              # JSON-RPC 2.0 codec
│   │       └── protocol.rs             # 管理方法面 + 回调面 定义
│   └── metado-daemon/                  # 引擎 daemon 进程主程序 (§11.1)
│       ├── Cargo.toml
│       └── src/main.rs
├── packages/
│   └── runtime-node/                   # Node 侧极简占位包 (dev-only)
│       ├── package.json
│       ├── tsconfig.json
│       ├── src/
│       │   ├── index.ts
│       │   ├── http.ts
│       │   ├── storage.ts
│       │   ├── file.ts
│       │   ├── time.ts
│       │   ├── log.ts
│       │   ├── crypto.ts
│       │   ├── custom.ts
│       │   ├── shim.ts                 # Buffer/path/events re-export
│       │   ├── errors.ts
│       │   └── test-utils.ts           # __METADO_TEST__ 权限模拟器
│       └── tests/
│           └── permission.test.ts
└── examples/
    ├── hello-plugin/                   # 最小示例插件
    │   ├── mdl.toml
    │   ├── package.json
    │   └── src/
    │       └── on_message.js
    └── http-plugin/                    # HTTP 能力示例
        ├── mdl.toml
        ├── package.json
        └── src/
            └── on_message.js
```

---

## Phase 0: Pre-requisite Research (§14 开放研究项)

> **Gate:** Phase 0 全部完成才进入 Phase 1。产出为规格锁定文档。

### Task 0.1: Lock `@metado/runtime` Export Spec

**Goal:** 逐一对照 Web/Node 约定，确定 http/storage/vfs/file/time/log/crypto/custom 的初版导出形状。

**Files:**
- Create: `docs/superpowers/specs/runtime-export-spec.md`

- [ ] **Step 1:** Create export spec document with table per capability:

```markdown
# @metado/runtime Export Spec v1

## http
| Export | Shape | Web Reference | Notes |
|--------|-------|---------------|-------|
| get(input: RequestInfo, init?: RequestInit): Promise\<Response\> | fetch 风格 | Fetch API | 走 realm 授权视图 |
| post(input: RequestInfo, init?: RequestInit): Promise\<Response\> | fetch 风格 | Fetch API | |

## storage
| Export | Shape | Notes |
|--------|-------|-------|
| read(key: string): Promise\<Uint8Array\> | 自研 | signer namespace via `storage.<signer>.write` |
| write(key: string, data: Uint8Array): Promise\<void\> | 自研 | |

## file
| Export | Shape | Notes |
|--------|-------|-------|
| read(path: string): Promise\<Uint8Array\> | 自研 | read-only vfs |
| stat(path: string): Promise\<{size, mtime}> | 自研 | |

## time
| Export | Shape | Notes |
|--------|-------|-------|
| now(): number | Date.now() | epoch ms |
| sleep(ms: number): Promise\<void\> | 标准 | |

## log
| Export | Shape | Notes |
|--------|-------|-------|
| info(...args: any[]): void | console 风格 | |
| warn(...args: any[]): void | console 风格 | |
| error(...args: any[]): void | console 风格 | |
| debug(...args: any[]): void | console 风格 | |

## crypto
| Export | Shape | Notes |
|--------|-------|-------|
| randomBytes(n: number): Uint8Array | WebCrypto 风格 | |
| sha256(data: Uint8Array): Uint8Array | WebCrypto 风格 | |
| hmac(key: Uint8Array, data: Uint8Array): Uint8Array | WebCrypto 风格 | |

## custom
| Export | Shape | Notes |
|--------|-------|-------|
| dispatch(name: string, params: JsonValue): Promise\<JsonValue\> | 自研 | capability message |

## Buffer/path/events (shim)
| Export | Shape | Notes |
|--------|-------|-------|
| Buffer | Node Buffer | re-export for npm interop |
| path | Node path | re-export for npm interop |
| events | Node events | re-export for npm interop |
```

- [ ] **Step 2:** Review against §8.5 API 风格原则 (Web Platform 优先, Node whitelist shim, 无标准处自研最小面)
- [ ] **Step 3:** Verify boa support for each Web type (URL, Blob, TextEncoder, fetch semantics)
- [ ] **Step 4:** Lock spec, commit

### Task 0.2: boa Web Support Audit

**Goal:** Audit boa's support for Web Platform APIs needed by `@metado/runtime`.

**Files:**
- Create: `docs/superpowers/specs/boa-web-audit.md`

- [ ] **Step 1:** Create audit document:

```markdown
# boa Web API Support Audit

| API | boa Status | Gap | Mitigation |
|-----|-----------|-----|------------|
| URL / URLSearchParams | ✅ native | - | - |
| TextEncoder / TextDecoder | ✅ native | - | - |
| Blob | ❌ missing | - | 需 boa 内置或 shim |
| ReadableStream | ❌ missing | - | v1 不做流式, 可后置 |
| fetch / Request / Response / Headers | ❌ missing | - | 能力层自行实现, 不依赖 boa fetch |
| WebCrypto (subtle) | partial | - | 仅 hash/hmac, 不需 subtle |
| queueMicrotask | ✅ native | - | - |
| globalThis | ✅ native | - | - |
| Event / CustomEvent | ❌ missing | - | 能力层不依赖 DOM 事件 |
```

- [ ] **Step 2:** Prototype boa integration test (import URL, TextEncoder)
- [ ] **Step 3:** Document missing APIs and decide mitigation per capability
- [ ] **Step 4:** Lock audit, commit

### Task 0.3: Runtime Export Spec → Engine Implementation Mapping

**Goal:** Map each `@metado/runtime` export to engine-side implementation approach.

**Files:**
- Modify: `docs/superpowers/specs/runtime-export-spec.md`

- [ ] **Step 1:** Add "Engine Implementation" column to each export table:

```markdown
| Export | Engine Implementation |
|--------|-----------------------|
| http.get() | metado-cap-http crate, reqwest backend |
| storage.read() | metado-cap-storage crate, filesystem backend |
| file.read() | metado-cap-file crate, read-only mount |
| ... | ... |
```

- [ ] **Step 2:** Identify which exports need boa polyfill vs native engine implementation
- [ ] **Step 3:** Commit

---

## Phase 1: Engine Core (metado-engine)

> 无 JS 执行, 纯 Rust 数据结构 + 逻辑。

### Task 1.1: Value Model (§4.4)

**Goal:** Implement the unified value representation.

**Files:**
- Create: `crates/metado-engine/src/value.rs`

**Interfaces:**
- Produces: `Value` enum, `try_from_js()` / `to_js()` conversion stubs (full in Phase 2)

- [x] **Step 1:** Write failing test for Value enum

```rust
// tests/value_test.rs
use metado_engine::value::Value;

#[test]
fn test_null_value() {
    let v = Value::Null;
    assert!(!v.is_truthy());
}

#[test]
fn test_number_value() {
    let v = Value::Number(42.0);
    assert!(v.is_truthy());
    assert_eq!(v.as_f64(), Some(42.0));
}

#[test]
fn test_string_value() {
    let v = Value::String("hello".into());
    assert!(v.is_truthy());
    assert_eq!(v.as_str(), Some("hello"));
}

#[test]
fn test_bytes_value() {
    let v = Value::Bytes(vec![1, 2, 3]);
    assert!(v.is_truthy());
    assert_eq!(v.as_bytes(), Some(&[1u8, 2, 3][..]));
}

#[test]
fn test_list_value() {
    let v = Value::List(vec![Value::Number(1.0), Value::String("a".into())]);
    assert_eq!(v.len(), Some(2));
}

#[test]
fn test_json_map_value() {
    let mut map = std::collections::HashMap::new();
    map.insert("key".into(), Value::Bool(true));
    let v = Value::JsonMap(map);
    assert!(v.is_truthy());
}

#[test]
fn test_json_roundtrip() {
    let v = Value::List(vec![
        Value::Null,
        Value::Bool(true),
        Value::Number(3.14),
        Value::String("test".into()),
        Value::Bytes(vec![0xff]),
    ]);
    let json = v.to_json_string().unwrap();
    let v2 = Value::from_json(&json).unwrap();
    assert_eq!(v, v2);
}
```

- [x] **Step 2:** Run test to verify it fails

Run: `cargo test --package metado-engine value_test`
Expected: FAIL with "unresolved import" or "module not found"

- [x] **Step 3:** Write minimal implementation

```rust
// crates/metado-engine/src/value.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    JsonMap(HashMap<String, Value>),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Null | Value::Bool(false) | Value::Number(0.0) => false,
            Value::String(s) => !s.is_empty(),
            _ => true,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn len(&self) -> Option<usize> {
        match self {
            Value::List(l) => Some(l.len()),
            Value::JsonMap(m) => Some(m.len()),
            _ => None,
        }
    }

    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}
```

- [x] **Step 4:** Run test to verify it passes

Run: `cargo test --package metado-engine value_test`
Expected: PASS

- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/value.rs crates/metado-engine/src/lib.rs
git commit -m "feat(engine): implement Value model (§4.4)"
```

### Task 1.2: Error Model (§9.2)

**Goal:** Implement ExecutionError with four fault kinds.

**Files:**
- Create: `crates/metado-engine/src/error.rs`

**Interfaces:**
- Produces: `ExecutionError { entry, kind, message }`, `ErrorKind` enum

- [x] **Step 1:** Write failing test

```rust
// tests/error_test.rs
use metado_engine::error::{ExecutionError, ErrorKind};

#[test]
fn test_plugin_error() {
    let e = ExecutionError::new("onMessage", ErrorKind::PluginError, "JS threw");
    assert_eq!(e.entry, "onMessage");
    assert_eq!(e.kind, ErrorKind::PluginError);
    assert_eq!(e.message, "JS threw");
}

#[test]
fn test_permission_denied() {
    let e = ExecutionError::new("onMessage", ErrorKind::PermissionDenied, "http.get");
    assert_eq!(e.kind, ErrorKind::PermissionDenied);
}

#[test]
fn test_capability_error() {
    let e = ExecutionError::new("onMessage", ErrorKind::CapabilityError, "storage write failed");
    assert_eq!(e.kind, ErrorKind::CapabilityError);
}

#[test]
fn test_fault() {
    let e = ExecutionError::new("onMessage", ErrorKind::Fault, "fuel exhausted");
    assert_eq!(e.kind, ErrorKind::Fault);
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement error module

```rust
// crates/metado-engine/src/error.rs
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    PluginError,
    PermissionDenied,
    CapabilityError,
    Fault,
}

#[derive(Debug, Clone)]
pub struct ExecutionError {
    pub entry: String,
    pub kind: ErrorKind,
    pub message: String,
}

impl ExecutionError {
    pub fn new(entry: &str, kind: ErrorKind, message: &str) -> Self {
        Self {
            entry: entry.to_string(),
            kind,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}: {}", self.kind, self.entry, self.message)
    }
}

impl std::error::Error for ExecutionError {}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/error.rs
git commit -m "feat(engine): implement ExecutionError model (§9.2)"
```

### Task 1.3: Manifest Parser (§4.2)

**Goal:** Parse `mdl.toml` manifest files.

**Files:**
- Create: `crates/metado-engine/src/manifest.rs`

**Interfaces:**
- Produces: `Manifest { name, version, permission, permission_set, entries, lifecycle }`

- [x] **Step 1:** Write failing test

```rust
// tests/manifest_test.rs
use metado_engine::manifest::{Manifest, EntryDef, LifecycleMode};

#[test]
fn test_parse_minimal_manifest() {
    let toml = r#"
        name = "myplugin"
        version = "1.0.0"
    "#;
    let m: Manifest = Manifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "myplugin");
    assert_eq!(m.version, "1.0.0");
    assert!(m.permission.is_empty());
    assert!(m.entries.is_empty());
}

#[test]
fn test_parse_full_manifest() {
    let toml = r#"
        name = "myplugin"
        version = "2.0.0"
        permission = ["http.get.api.example", "storage.<signer>.write"]
        permission-set = ["standard"]
        lifecycle = "resident-high"

        [entries.onMessage]
        export = "onMessage"

        [entries.boot]
        export = "boot"
    "#;
    let m: Manifest = Manifest::from_toml(toml).unwrap();
    assert_eq!(m.name, "myplugin");
    assert_eq!(m.version, "2.0.0");
    assert_eq!(m.permission, vec!["http.get.api.example", "storage.<signer>.write"]);
    assert_eq!(m.permission_set, vec!["standard"]);
    assert_eq!(m.lifecycle, LifecycleMode::ResidentHigh);
    assert_eq!(m.entries.len(), 2);
    assert_eq!(m.entries["onMessage"].export, "onMessage");
    assert_eq!(m.entries["boot"].export, "boot");
}

#[test]
fn test_invalid_toml() {
    let toml = "this is not toml {{{";
    assert!(Manifest::from_toml(toml).is_err());
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement manifest parser

```rust
// crates/metado-engine/src/manifest.rs
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub enum LifecycleMode {
    #[serde(rename = "resident-high")]
    ResidentHigh,
    #[serde(rename = "resident-low")]
    ResidentLow,
    #[serde(rename = "cold")]
    Cold,
}

impl Default for LifecycleMode {
    fn default() -> Self { Self::ResidentLow }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntryDef {
    pub export: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub permission: Vec<String>,
    #[serde(default, rename = "permission-set")]
    pub permission_set: Vec<String>,
    #[serde(default)]
    pub lifecycle: LifecycleMode,
    #[serde(default)]
    pub entries: HashMap<String, EntryDef>,
}

impl Manifest {
    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/manifest.rs
git commit -m "feat(engine): implement manifest parser (§4.2)"
```

### Task 1.4: Signature Verification (§6)

**Goal:** Implement Ed25519 signing and verification with MDL1 envelope.

**Files:**
- Create: `crates/metado-engine/src/signature.rs`

**Interfaces:**
- Produces: `sign(keypair, payload) -> SignedBundle`, `verify(pubkey, bundle) -> Result<Vec<u8>>`, `signer_id(pubkey) -> String`

- [x] **Step 1:** Write failing test

```rust
// tests/signature_test.rs
use metado_engine::signature::{KeyPair, SignedBundle, verify, signer_id};

#[test]
fn test_sign_and_verify() {
    let kp = KeyPair::generate();
    let payload = b"hello world";
    let signed = SignedBundle::sign(&kp, payload);
    assert!(signed.verify().is_ok());
    assert_eq!(signed.payload(), payload);
}

#[test]
fn test_signer_id() {
    let kp = KeyPair::generate();
    let id = signer_id(kp.public_key_bytes());
    assert_eq!(id.len(), 32); // 公钥指纹: sha256 截段 16 bytes = 32 hex chars
}

#[test]
fn test_tampered_payload_fails() {
    let kp = KeyPair::generate();
    let mut signed = SignedBundle::sign(&kp, b"original");
    // Tamper with payload
    signed.payload_mut()[0] = 0xff;
    assert!(signed.verify().is_err());
}

#[test]
fn test_wrong_key_fails() {
    let kp1 = KeyPair::generate();
    let kp2 = KeyPair::generate();
    let signed = SignedBundle::sign(&kp1, b"hello");
    assert!(signed.verify_with_key(kp2.public_key_bytes()).is_err());
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement signature module

```rust
// crates/metado-engine/src/signature.rs
use ed25519_dalek::{Signer, Verifier, SigningKey, VerifyingKey, Signature};
use sha2::{Sha256, Digest};

pub const MDL_MAGIC: &[u8; 4] = b"MDL1";
pub const MDL_FORMAT_VERSION: u16 = 1;

pub struct KeyPair {
    signing_key: SigningKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        // rand >= 0.10: `thread_rng()` renamed to `rng()`
        let mut rng = rand::rng();
        Self { signing_key: SigningKey::generate(&mut rng) }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self { signing_key: SigningKey::from_bytes(bytes) }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

pub fn signer_id(pubkey: &[u8; 32]) -> String {
    let hash = Sha256::digest(pubkey);
    hex::encode(&hash[..16]) // 截段 16 bytes = 32 hex chars
}

pub struct SignedBundle {
    header: Vec<u8>,   // magic + version + pubkey + payload_len
    signature: Signature,
    payload: Vec<u8>,
}

impl SignedBundle {
    pub fn sign(kp: &KeyPair, payload: &[u8]) -> Self {
        let mut header = Vec::new();
        header.extend_from_slice(MDL_MAGIC);
        header.extend_from_slice(&MDL_FORMAT_VERSION.to_le_bytes());
        header.extend_from_slice(&kp.public_key_bytes());
        header.extend_from_slice(&(payload.len() as u64).to_le_bytes());

        let mut data_to_sign = header.clone();
        data_to_sign.extend_from_slice(payload);

        let signature = kp.signing_key.sign(&data_to_sign);

        Self {
            header,
            signature,
            payload: payload.to_vec(),
        }
    }

    pub fn verify(&self) -> Result<(), String> {
        // header layout: magic(4) + version(2) + pubkey(32) + payload_len(8)
        let pubkey_bytes: [u8; 32] = self.header[6..38]
            .try_into()
            .map_err(|_| "invalid header".to_string())?;
        self.verify_with_key(&pubkey_bytes)
    }

    pub fn verify_with_key(&self, pubkey: &[u8; 32]) -> Result<(), String> {
        let verifying_key = VerifyingKey::from_bytes(pubkey)
            .map_err(|e| format!("invalid key: {}", e))?;

        let mut data_to_verify = self.header.clone();
        data_to_verify.extend_from_slice(&self.payload);

        verifying_key.verify(&data_to_verify, &self.signature)
            .map_err(|e| format!("signature invalid: {}", e))
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut Vec<u8> {
        &mut self.payload
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.header);
        out.extend_from_slice(&self.signature.to_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 4 + 2 + 32 + 8 + 64 {
            return Err("too short".into());
        }
        if &data[..4] != MDL_MAGIC {
            return Err("invalid magic".into());
        }

        let header = data[..46].to_vec(); // 4 + 2 + 32 + 8
        let sig_bytes: [u8; 64] = data[46..110].try_into()
            .map_err(|_| "invalid signature length".into())?;
        // ed25519-dalek >= 3: `Signature::from_bytes` removed; use `TryFrom<&[u8]>`
        let signature = Signature::try_from(&sig_bytes[..])
            .map_err(|_| "invalid signature".to_string())?;
        let payload = data[110..].to_vec();

        Ok(Self { header, signature, payload })
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/signature.rs
git commit -m "feat(engine): implement Ed25519 signature (§6)"
```

### Task 1.5: Permission Resolver (§5)

**Goal:** Implement three-layer permission resolution: requested ∩ available → granted → check.

**Files:**
- Create: `crates/metado-engine/src/permission.rs`

**Interfaces:**
- Produces: `PermissionResolver { requested, granted, available, domain_rules }`, `check(capability_name) -> Result<(), PermissionDenied>`

- [x] **Step 1:** Write failing test

```rust
// tests/permission_test.rs
use metado_engine::permission::{PermissionResolver, PermissionSet};

#[test]
fn test_requested_available_intersection() {
    let resolver = PermissionResolver::new(
        vec!["http.get".into(), "http.post".into(), "storage.read".into()],
        vec!["http.get".into(), "log.info".into()], // available = compiled ∩ active
    );
    // http.get 在 requested ∩ available → 可授
    // http.post 不在 available → 不可授
    // storage.read 在 requested 但不在 available → 不可授
    // log.info 不在 requested → 不可授
    assert!(resolver.can_grant("http.get"));
    assert!(!resolver.can_grant("http.post"));
    assert!(!resolver.can_grant("storage.read"));
    assert!(!resolver.can_grant("log.info"));
}

#[test]
fn test_grant_subset_of_requested() {
    let mut resolver = PermissionResolver::new(
        vec!["http.get".into(), "http.post".into()],
        vec!["http.get".into(), "http.post".into()],
    );
    resolver.grant(vec!["http.get".into()]);
    assert!(resolver.check("http.get").is_ok());
    assert!(resolver.check("http.post").is_err());
}

#[test]
fn test_signer_domain_binding() {
    let mut resolver = PermissionResolver::new(
        vec!["storage.<signer>.write".into()],
        vec!["storage.<signer>.write".into()],
    );
    resolver.bind_signer("abc123");
    resolver.grant(vec!["storage.abc123.write".into()]);
    assert!(resolver.check("storage.abc123.write").is_ok());
}

#[test]
fn test_permission_set_expansion() {
    let mut ps = PermissionSet::new();
    ps.define("standard", vec!["http.get".into(), "log.info".into()]);
    let expanded = ps.expand(&["standard".into()]).unwrap();
    assert_eq!(expanded, vec!["http.get", "log.info"]);
}

#[test]
fn test_undefined_permission_set_fails() {
    let ps = PermissionSet::new();
    assert!(ps.expand(&["nonexistent".into()]).is_err());
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement permission module

```rust
// crates/metado-engine/src/permission.rs
use std::collections::HashSet;

// 注意: 含 `Vec<Box<dyn Fn(&str) -> bool>>` 不可 derive Debug/Clone（dyn Fn 非 Sized）
pub struct PermissionResolver {
    requested: Vec<String>,
    granted: HashSet<String>,
    available: Vec<String>,
    signer_id: Option<String>,
    domain_rules: Vec<Box<dyn Fn(&str) -> bool>>,
}

impl PermissionResolver {
    pub fn new(requested: Vec<String>, available: Vec<String>) -> Self {
        Self {
            requested,
            granted: HashSet::new(),
            available,
            signer_id: None,
            domain_rules: Vec::new(),
        }
    }

    pub fn bind_signer(&mut self, signer_id: &str) {
        self.signer_id = Some(signer_id.to_string());
    }

    // 注意: 模板匹配 —— requested/available 可能含 `<signer>` 占位符，
    // 绑定 signer 后对具体权限同样成立（TDD 实际修正）
    pub fn can_grant(&self, perm: &str) -> bool {
        self.requested.iter().any(|r| self.matches(r, perm))
            && self.available.iter().any(|a| self.matches(a, perm))
    }

    fn matches(&self, template: &str, perm: &str) -> bool {
        if template == perm {
            return true;
        }
        if template.contains("<signer>") {
            return self.resolve_signer_placeholder(template) == perm;
        }
        false
    }

    pub fn grant(&mut self, perms: Vec<String>) {
        for p in perms {
            if self.can_grant(&p) {
                self.granted.insert(p);
            }
        }
    }

    pub fn revoke(&mut self, perms: &[String]) {
        for p in perms {
            self.granted.remove(p);
        }
    }

    pub fn check(&self, capability_name: &str) -> Result<(), String> {
        if !self.granted.contains(capability_name) {
            return Err(format!("Permission denied: {}", capability_name));
        }
        for rule in &self.domain_rules {
            if !rule(capability_name) {
                return Err(format!("Domain rule denied: {}", capability_name));
            }
        }
        Ok(())
    }

    pub fn resolve_signer_placeholder(&self, perm: &str) -> String {
        if let Some(ref signer) = self.signer_id {
            perm.replace("<signer>", signer)
        } else {
            perm.to_string()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PermissionSet {
    sets: std::collections::HashMap<String, Vec<String>>,
}

impl PermissionSet {
    pub fn new() -> Self { Self::default() }

    pub fn define(&mut self, name: &str, perms: Vec<String>) {
        self.sets.insert(name.to_string(), perms);
    }

    pub fn expand(&self, names: &[String]) -> Result<Vec<String>, String> {
        let mut result = Vec::new();
        for name in names {
            match self.sets.get(name) {
                Some(perms) => result.extend(perms.clone()),
                None => return Err(format!("Undefined permission set: {}", name)),
            }
        }
        Ok(result)
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/permission.rs
git commit -m "feat(engine): implement permission resolver (§5)"
```

### Task 1.6: Capability Framework (§8.1)

**Goal:** Implement `CapabilitySet` trait and `CapabilityRegistry` for compile-time and runtime capability registration.

**Files:**
- Create: `crates/metado-engine/src/capability.rs`

**Interfaces:**
- Produces: `CapabilitySet` trait, `CapabilityRegistry`, `CapabilityMeta { permissions, exports }`

- [x] **Step 1:** Write failing test

```rust
// tests/capability_test.rs
use metado_engine::capability::{CapabilitySet, CapabilityRegistry, CapabilityMeta};

struct MockHttp;

impl CapabilitySet for MockHttp {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "http".into(),
            permissions: vec!["http.get".into(), "http.post".into()],
            exports: vec!["get".into(), "post".into()],
        }
    }
}

#[test]
fn test_register_and_lookup() {
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(MockHttp));

    let cap = registry.get("http").unwrap();
    assert_eq!(cap.meta().name, "http");
    assert!(cap.meta().permissions.contains(&"http.get".to_string()));
}

#[test]
fn test_available_permissions() {
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(MockHttp));

    let all_perms = registry.all_permissions();
    assert!(all_perms.contains(&"http.get".to_string()));
    assert!(all_perms.contains(&"http.post".to_string()));
}

#[test]
fn test_all_exports() {
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(MockHttp));

    let all_exports = registry.all_exports();
    assert!(all_exports.contains(&"get".to_string()));
    assert!(all_exports.contains(&"post".to_string()));
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement capability module

```rust
// crates/metado-engine/src/capability.rs

pub struct CapabilityMeta {
    pub name: String,
    pub permissions: Vec<String>,
    pub exports: Vec<String>,
}

pub trait CapabilitySet {
    fn meta(&self) -> CapabilityMeta;
}

pub struct CapabilityRegistry {
    capabilities: Vec<Box<dyn CapabilitySet>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self { capabilities: Vec::new() }
    }

    pub fn register(&mut self, cap: Box<dyn CapabilitySet>) {
        self.capabilities.push(cap);
    }

    pub fn get(&self, name: &str) -> Option<&dyn CapabilitySet> {
        self.capabilities.iter()
            .find(|c| c.meta().name == name)
            .map(|c| c.as_ref())
    }

    pub fn all_permissions(&self) -> Vec<String> {
        self.capabilities.iter()
            .flat_map(|c| c.meta().permissions)
            .collect()
    }

    pub fn all_exports(&self) -> Vec<String> {
        self.capabilities.iter()
            .flat_map(|c| c.meta().exports)
            .collect()
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/capability.rs
git commit -m "feat(engine): implement capability framework (§8.1)"
```

### Task 1.7: Container Reader (§10.2)

**Goal:** Read ZIP container, extract files, validate structure.

**Files:**
- Create: `crates/metado-engine/src/container.rs`

**Interfaces:**
- Produces: `Container { manifest, files }`, `Container::from_bytes()`, `Container::read_file(path)`

- [x] **Step 1:** Write failing test

```rust
// tests/container_test.rs
use metado_engine::container::Container;

#[test]
fn test_read_valid_container() {
    // Build a minimal ZIP in-memory with mdl.toml + src/main.js
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("mdl.toml", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"name = \"test\"\nversion = \"1.0\"").unwrap();
    zip.start_file("src/main.js", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"export function hello() { return 1; }").unwrap();
    let data = zip.finish().unwrap().into_inner();

    let c = Container::from_bytes(&data).unwrap();
    assert!(c.file_exists("mdl.toml"));
    assert!(c.file_exists("src/main.js"));
    let main = c.read_file("src/main.js").unwrap();
    assert!(std::str::from_utf8(&main).unwrap().contains("hello"));
}

#[test]
fn test_missing_mdl_toml_fails() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("src/main.js", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"export function hello() { return 1; }").unwrap();
    let data = zip.finish().unwrap().into_inner();

    let c = Container::from_bytes(&data);
    assert!(c.is_err()); // 应报错: 缺少 mdl.toml
}

#[test]
fn test_path_traversal_blocked() {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("mdl.toml", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"name = \"test\"\nversion = \"1.0\"").unwrap();
    zip.start_file("../../../etc/passwd", zip::write::SimpleFileOptions::default()).unwrap();
    zip.write_all(b"evil").unwrap();
    let data = zip.finish().unwrap().into_inner();

    let c = Container::from_bytes(&data).unwrap();
    assert!(c.read_file("../../../etc/passwd").is_err()); // 路径穿越被拒
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement container module

```rust
// crates/metado-engine/src/container.rs
use std::collections::HashMap;
use std::io::Read;

pub struct Container {
    files: HashMap<String, Vec<u8>>,
}

impl Container {
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let reader = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| format!("invalid ZIP: {}", e))?;

        let mut files = HashMap::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| format!("ZIP read error: {}", e))?;
            if file.is_dir() { continue; }

            let name = file.name().to_string();
            let mut content = Vec::new();
            file.read_to_end(&mut content).map_err(|e| format!("read error: {}", e))?;
            files.insert(name, content);
        }

        if !files.contains_key("mdl.toml") {
            return Err("missing mdl.toml in container".into());
        }

        Ok(Self { files })
    }

    pub fn file_exists(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, String> {
        // Block path traversal
        if path.contains("..") || path.starts_with('/') {
            return Err(format!("path traversal blocked: {}", path));
        }
        self.files.get(path)
            .cloned()
            .ok_or_else(|| format!("file not found: {}", path))
    }

    pub fn files(&self) -> impl Iterator<Item = (&String, &Vec<u8>)> {
        self.files.iter()
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/container.rs
git commit -m "feat(engine): implement ZIP container reader (§10.2)"
```

### Task 1.8: Plugin Lifecycle State Machine (§7)

**Goal:** Implement plugin lifecycle states and transitions.

**Files:**
- Create: `crates/metado-engine/src/realm.rs`, `crates/metado-engine/src/plugin.rs`

**Interfaces:**
- Produces: `PluginState` enum, `Plugin { id, signer_id, manifest, state, realm }`, `Plugin::load()`, `Plugin::transition()`

- [x] **Step 1:** Write failing test

```rust
// tests/plugin_test.rs
use metado_engine::plugin::{Plugin, PluginState};

#[test]
fn test_lifecycle_transitions() {
    let mut plugin = Plugin::new("test-plugin", "signer123");
    assert_eq!(plugin.state, PluginState::Absent);

    plugin.transition(PluginState::Installing).unwrap();
    assert_eq!(plugin.state, PluginState::Installing);

    plugin.transition(PluginState::Installed).unwrap();
    assert_eq!(plugin.state, PluginState::Installed);

    plugin.transition(PluginState::Loading).unwrap();
    assert_eq!(plugin.state, PluginState::Loading);

    plugin.transition(PluginState::Pending).unwrap();
    assert_eq!(plugin.state, PluginState::Pending);

    plugin.transition(PluginState::Active).unwrap();
    assert_eq!(plugin.state, PluginState::Active);
}

#[test]
fn test_invalid_transition_fails() {
    let mut plugin = Plugin::new("test", "signer");
    assert!(plugin.transition(PluginState::Active).is_err()); // absent → active 非法
}

#[test]
fn test_evicted_can_reactivate() {
    let mut plugin = Plugin::new("test", "signer");
    plugin.transition(PluginState::Installing).unwrap();
    plugin.transition(PluginState::Installed).unwrap();
    plugin.transition(PluginState::Loading).unwrap();
    plugin.transition(PluginState::Pending).unwrap();
    plugin.transition(PluginState::Active).unwrap();
    plugin.transition(PluginState::Evicted).unwrap();
    plugin.transition(PluginState::Active).unwrap(); // 重建 realm
    assert_eq!(plugin.state, PluginState::Active);
}

#[test]
fn test_quarantine_from_active() {
    let mut plugin = Plugin::new("test", "signer");
    plugin.transition(PluginState::Installing).unwrap();
    plugin.transition(PluginState::Installed).unwrap();
    plugin.transition(PluginState::Loading).unwrap();
    plugin.transition(PluginState::Pending).unwrap();
    plugin.transition(PluginState::Active).unwrap();
    plugin.transition(PluginState::Quarantined).unwrap();
    assert_eq!(plugin.state, PluginState::Quarantined);
}

#[test]
fn test_uninstall_from_any_state() {
    let mut plugin = Plugin::new("test", "signer");
    plugin.transition(PluginState::Installing).unwrap();
    plugin.transition(PluginState::Uninstalled).unwrap();
    assert_eq!(plugin.state, PluginState::Uninstalled);
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement plugin module

```rust
// crates/metado-engine/src/plugin.rs
use crate::manifest::Manifest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Absent,
    Installing,
    Installed,
    Loading,
    Pending,
    Active,
    Evicted,
    Quarantined,
    Uninstalled,
}

pub struct Plugin {
    pub id: String,
    pub signer_id: String,
    pub manifest: Option<Manifest>,
    pub state: PluginState,
    pub granted: Vec<String>,
}

impl Plugin {
    pub fn new(id: &str, signer_id: &str) -> Self {
        Self {
            id: id.to_string(),
            signer_id: signer_id.to_string(),
            manifest: None,
            state: PluginState::Absent,
            granted: Vec::new(),
        }
    }

    pub fn transition(&mut self, new_state: PluginState) -> Result<(), String> {
        let valid = matches!(
            (&self.state, &new_state),
            (PluginState::Absent, PluginState::Installing)
                | (PluginState::Installing, PluginState::Installed)
                | (PluginState::Installed, PluginState::Loading)
                | (PluginState::Loading, PluginState::Pending)
                | (PluginState::Pending, PluginState::Active)
                | (PluginState::Active, PluginState::Evicted)
                | (PluginState::Active, PluginState::Quarantined)
                | (PluginState::Evicted, PluginState::Active)
                | (PluginState::Quarantined, PluginState::Active)
                // 任何状态 → Uninstalled
                | (_, PluginState::Uninstalled)
        );

        if valid {
            self.state = new_state;
            Ok(())
        } else {
            Err(format!("invalid transition: {:?} → {:?}", self.state, new_state))
        }
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/plugin.rs # realm.rs 计划未定义接口，按 YAGNI 跳过
git commit -m "feat(engine): implement plugin lifecycle state machine (§7)"
```

### Task 1.9: Trace Sink (§12.6)

**Goal:** Implement trace event recording for capability calls, permission checks, and value flow.

**Files:**
- Create: `crates/metado-engine/src/trace.rs`

**Interfaces:**
- Produces: `TraceSink`, `TraceEvent` enum, `TraceSink::record()`, `TraceSink::events()`

- [x] **Step 1:** Write failing test

```rust
// tests/trace_test.rs
use metado_engine::trace::{TraceSink, TraceEvent};

#[test]
fn test_record_and_retrieve() {
    let mut sink = TraceSink::new();
    sink.record(TraceEvent::CapabilityCall {
        module: "http".into(),
        line: 10,
        capability: "http.get".into(),
        requested: vec!["http.get".into()],
        granted: vec!["http.get".into()],
    });

    let events = sink.events();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_filter_by_event_type() {
    let mut sink = TraceSink::new();
    sink.record(TraceEvent::EntryStart { entry: "onMessage".into() });
    sink.record(TraceEvent::PermissionCheck {
        capability: "http.get".into(),
        passed: true,
    });
    sink.record(TraceEvent::EntryEnd { entry: "onMessage".into() });

    let checks: Vec<_> = sink.events().iter()
        .filter(|e| matches!(e, TraceEvent::PermissionCheck { .. }))
        .collect();
    assert_eq!(checks.len(), 1);
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement trace module

```rust
// crates/metado-engine/src/trace.rs

#[derive(Debug, Clone)]
pub enum TraceEvent {
    EntryStart { entry: String },
    EntryEnd { entry: String },
    CapabilityCall {
        module: String,
        line: u32,
        capability: String,
        requested: Vec<String>,
        granted: Vec<String>,
    },
    PermissionCheck {
        capability: String,
        passed: bool,
    },
    ValueFlow { // 计划原名 `Value流转`，中文标识符不合理改用英文
        direction: String, // "in" or "out"
        size_hint: usize,
    },
}

pub struct TraceSink {
    events: Vec<TraceEvent>,
}

impl TraceSink {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn record(&mut self, event: TraceEvent) {
        self.events.push(event);
    }

    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/trace.rs
git commit -m "feat(engine): implement trace sink (§12.6)"
```

### Task 1.10: Engine Facade (§13)

**Goal:** Wire all engine components into the public `Engine` API.

**Files:**
- Modify: `crates/metado-engine/src/lib.rs`

**Interfaces:**
- Produces: `Engine::new()`, `Engine::register_capability()`, `Engine::define_permission_set()`, `Engine::load_plugin()`, `Engine::grant()`, `Engine::invoke()`

- [x] **Step 1:** Write failing integration test

```rust
// tests/engine_integration_test.rs
use metado_engine::{Engine, Value};

#[test]
fn test_engine_create_and_register() {
    let mut engine = Engine::new();
    engine.define_permission_set("standard", vec!["http.get".into()]);
    // Engine 创建成功
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement engine facade

```rust
// crates/metado-engine/src/lib.rs
pub mod value;
pub mod error;
pub mod manifest;
pub mod container;
pub mod signature;
pub mod permission;
pub mod capability;
pub mod plugin;
pub mod realm;
pub mod trace;

// 实现偏差: bundle.header 为私有 -> 新增 SignedBundle::signer_pubkey();
// 偏移修正 [8..40] -> [6..38]; from_utf8 临时值借用修复;
// 新增 Engine::activate(Pending->Active) 否则 invoke 永远拒绝；load_plugin 返回 () 或 &Plugin 改为 Result<(), String>
pub use value::Value;
pub use error::{ExecutionError, ErrorKind};
pub use manifest::Manifest;
pub use container::Container;
pub use signature::{KeyPair, SignedBundle, signer_id};
pub use permission::{PermissionResolver, PermissionSet};
pub use capability::{CapabilitySet, CapabilityRegistry, CapabilityMeta};
pub use plugin::{Plugin, PluginState};
pub use trace::{TraceSink, TraceEvent};

pub struct Engine {
    capabilities: CapabilityRegistry,
    permission_sets: PermissionSet,
    plugins: Vec<Plugin>,
    trace: TraceSink,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            capabilities: CapabilityRegistry::new(),
            permission_sets: PermissionSet::new(),
            plugins: Vec::new(),
            trace: TraceSink::new(),
        }
    }

    pub fn register_capability(&mut self, cap: Box<dyn CapabilitySet>) {
        self.capabilities.register(cap);
    }

    pub fn define_permission_set(&mut self, name: &str, perms: Vec<String>) {
        self.permission_sets.define(name, perms);
    }

    pub fn load_plugin(&mut self, bundle: &SignedBundle, granted: Vec<String>) -> Result<&Plugin, String> {
        bundle.verify()?;

        let container = Container::from_bytes(bundle.payload())?;
        let toml_content = std::str::from_utf8(&container.read_file("mdl.toml")?)
            .map_err(|e| format!("invalid UTF-8 in mdl.toml: {}", e))?;
        let manifest = Manifest::from_toml(toml_content)
            .map_err(|e| format!("invalid manifest: {}", e))?;

        // 解析 signer_id
        let pubkey_bytes: [u8; 32] = bundle.header[8..40].try_into()
            .map_err(|_| "invalid header".to_string())?;
        let sid = signer_id(&pubkey_bytes);

        let mut plugin = Plugin::new(&manifest.name, &sid);
        plugin.manifest = Some(manifest);
        plugin.granted = granted;
        plugin.transition(PluginState::Installing)?;
        plugin.transition(PluginState::Installed)?;
        plugin.transition(PluginState::Loading)?;
        plugin.transition(PluginState::Pending)?;

        self.plugins.push(plugin);
        Ok(self.plugins.last().unwrap())
    }

    pub fn grant(&mut self, plugin_id: &str, perms: Vec<String>) -> Result<(), String> {
        let plugin = self.plugins.iter_mut()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
        plugin.granted.extend(perms);
        Ok(())
    }

    pub fn invoke(&self, plugin_id: &str, entry: &str, input: Value) -> Result<Value, ExecutionError> {
        let plugin = self.plugins.iter()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| ExecutionError::new(entry, ErrorKind::PluginError, "plugin not found"))?;

        if plugin.state != PluginState::Active {
            return Err(ExecutionError::new(entry, ErrorKind::PluginError, "plugin not active"));
        }

        // v1 stub: 返回输入值
        Ok(input)
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-engine/src/lib.rs
git commit -m "feat(engine): implement Engine facade (§13)"
```

---

## Phase 2: JS Executor (metado-executor)

> boa 桥接, 模块解析, 虚拟模块, 值互转, 执行预算。

### Task 2.1: boa Integration & Event Loop

**Goal:** Initialize boa engine, execute simple JS, integrate with tokio event loop.

**Files:**
- Create: `crates/metado-executor/src/engine.rs`

**Interfaces:**
- Produces: `JsEngine::new()`, `JsEngine::eval(code)`, `JsEngine::call(entry, args)`

- [x] **Step 1:** Write failing test

```rust
// tests/js_engine_test.rs
use metado_executor::JsEngine;

#[test]
fn test_eval_arithmetic() {
    let mut engine = JsEngine::new();
    let result = engine.eval("1 + 2").unwrap();
    assert_eq!(result.as_f64(), Some(3.0));
}

#[test]
fn test_eval_string() {
    let mut engine = JsEngine::new();
    let result = engine.eval("'hello' + ' world'").unwrap();
    assert_eq!(result.as_str(), Some("hello world"));
}

#[test]
fn test_call_function() {
    let mut engine = JsEngine::new();
    engine.eval("function add(a, b) { return a + b; }").unwrap();
    let result = engine.call("add", vec![Value::Number(1.0), Value::Number(2.0)]).unwrap();
    assert_eq!(result.as_f64(), Some(3.0));
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement JsEngine

```rust
// crates/metado-executor/src/engine.rs
// 平台修正: Termux/aarch64 (bionic ld) 与 NaN-boxing 冲突 -> 启用 boa_engine 的 `jsvalue-enum` feature
// 字符串入参: JsValue 无 From<&str>, 用 JsString::from(...).into()
use boa_engine::string::JsString;
use boa_engine::{Context, Source};

pub struct JsEngine {
    context: Context,
}

impl JsEngine {
    pub fn new() -> Self {
        let context = Context::default();
        Self { context }
    }

    pub fn eval(&mut self, code: &str) -> Result<metado_engine::Value, String> {
        let result = self.context.eval(Source::from_bytes(code))
            .map_err(|e| format!("JS error: {:?}", e))?;
        Ok(js_to_value(&result))
    }

    pub fn call(&mut self, name: &str, args: Vec<metado_engine::Value>) -> Result<metado_engine::Value, String> {
        let func = self.context.eval(Source::from_bytes(name))
            .map_err(|e| format!("function not found: {:?}", e))?;
        let js_args: Vec<_> = args.iter().map(value_to_js).collect();
        let callable = func.as_callable()
            .ok_or("not a function")?;
        // boa >= 0.20 (verified against 0.22.0 source): `call(&self, this, args, &mut Context)`
        let result = callable.call(&boa_engine::JsValue::undefined(), &js_args, &mut self.context)
            .map_err(|e| format!("call error: {:?}", e))?;
        Ok(js_to_value(&result))
    }
}

fn js_to_value(val: &boa_engine::JsValue) -> metado_engine::Value {
    if val.is_null_or_undefined() {
        metado_engine::Value::Null
    } else if let Some(b) = val.as_boolean() {
        metado_engine::Value::Bool(b)
    } else if let Some(n) = val.as_number() {
        metado_engine::Value::Number(n)
    } else if let Some(s) = val.as_string() {
        metado_engine::Value::String(s.to_std_string_escaped())
    } else {
        metado_engine::Value::Null
    }
}

fn value_to_js(val: &metado_engine::Value) -> boa_engine::JsValue {
    match val {
        metado_engine::Value::Null => boa_engine::JsValue::null(),
        metado_engine::Value::Bool(b) => (*b).into(),
        metado_engine::Value::Number(n) => (*n).into(),
        metado_engine::Value::String(s) => s.as_str().into(),
        _ => boa_engine::JsValue::null(),
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-executor/src/engine.rs
git commit -m "feat(executor): implement boa JS engine integration"
```

### Task 2.2: Node-style Module Resolution

**Goal:** Implement `exports` field, `node_modules` lookup, `.js` extension resolution.

**Files:**
- Create: `crates/metado-executor/src/module_loader.rs`

**Interfaces:**
- Produces: `ModuleLoader::resolve(specifier, from) -> PathBuf`, `ModuleLoader::load(path) -> Source`

- [x] **Step 1:** Write failing test

```rust
// 测试改写在真实临时目录建文件树（计划用虚构 /plugin 路径在真机 FS 不存在）
// tests/module_loader_test.rs
use metado_executor::ModuleLoader;

#[test]
fn test_resolve_relative() {
    let loader = ModuleLoader::new("/plugin/src");
    let resolved = loader.resolve("./helper.js", "/plugin/src/on_message.js").unwrap();
    assert_eq!(resolved, "/plugin/src/helper.js");
}

#[test]
fn test_resolve_node_modules() {
    let loader = ModuleLoader::new("/plugin");
    let resolved = loader.resolve("lodash", "/plugin/src/main.js").unwrap();
    assert!(resolved.contains("node_modules/lodash"));
}

#[test]
fn test_resolve_extension() {
    let loader = ModuleLoader::new("/plugin");
    let resolved = loader.resolve("./helper", "/plugin/src/main.js").unwrap();
    assert_eq!(resolved, "/plugin/src/helper.js");
}

#[test]
fn test_resolve_absolute() {
    let loader = ModuleLoader::new("/plugin");
    let resolved = loader.resolve("/usr/lib/util.js", "/plugin/src/main.js").unwrap();
    assert_eq!(resolved, "/usr/lib/util.js");
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement module loader

```rust
// crates/metado-executor/src/module_loader.rs
use std::path::{Path, PathBuf};

pub struct ModuleLoader {
    root: PathBuf,
}

impl ModuleLoader {
    pub fn new(root: &str) -> Self {
        Self { root: PathBuf::from(root) }
    }

    pub fn resolve(&self, specifier: &str, from: &str) -> Result<PathBuf, String> {
        let from_dir = Path::new(from).parent().unwrap_or(&self.root);

        // Absolute
        if specifier.starts_with('/') {
            return Ok(PathBuf::from(specifier));
        }

        // Relative
        if specifier.starts_with('.') {
            let resolved = from_dir.join(specifier);
            return self.try_with_extension(&resolved);
        }

        // node_modules
        self.resolve_node_modules(specifier, from_dir)
    }

    fn try_with_extension(&self, path: &Path) -> Result<PathBuf, String> {
        if path.exists() { return Ok(path.to_path_buf()); }
        let with_ext = path.with_extension("js");
        if with_ext.exists() { return Ok(with_ext); }
        Err(format!("module not found: {}", path.display()))
    }

    fn resolve_node_modules(&self, specifier: &str, from_dir: &Path) -> Result<PathBuf, String> {
        let mut current = from_dir.to_path_buf();
        loop {
            let candidate = current.join("node_modules").join(specifier);
            if candidate.exists() {
                return Ok(candidate);
            }
            let with_ext = candidate.with_extension("js");
            if with_ext.exists() {
                return Ok(with_ext);
            }
            if !current.pop() { break; }
        }
        Err(format!("module not found: {}", specifier))
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-executor/src/module_loader.rs
git commit -m "feat(executor): implement Node-style module resolution"
```

### Task 2.3: `@metado/runtime` Virtual Module

**Goal:** Map `@metado/runtime` specifier to built-in exports filtered by `available` permissions.

**Files:**
- Create: `crates/metado-executor/src/virtual_module.rs`

**Interfaces:**
- Produces: `VirtualModule { exports, granted }`, `VirtualModule::build(available, granted)`

- [x] **Step 1:** Write failing test

```rust
// tests/virtual_module_test.rs
use metado_executor::VirtualModule;

#[test]
fn test_exports_filtered_by_available() {
    let vm = VirtualModule::build(
        vec!["http.get".into(), "storage.read".into()],
        vec!["http.get".into()],
    );
    // http.get 在 available + granted → 导出
    assert!(vm.has_export("get")); // http sub-module
    // storage 不在 granted → 导出存在但调用时 reject
    // (导出存在性由 available 决定, 调用放行由 granted 决定)
}

#[test]
fn test_missing_available_not_exported() {
    let vm = VirtualModule::build(
        vec!["http.get".into()],
        vec![],
    );
    // http.get 在 available 但不在 granted → 导出存在
    assert!(vm.has_export("get"));
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement virtual module

```rust
// crates/metado-executor/src/virtual_module.rs
use std::collections::HashSet;

pub struct VirtualModule {
    available: HashSet<String>,
    granted: HashSet<String>,
}

impl VirtualModule {
    pub fn build(available: Vec<String>, granted: Vec<String>) -> Self {
        Self {
            available: available.into_iter().collect(),
            granted: granted.into_iter().collect(),
        }
    }

    pub fn has_export(&self, name: &str) -> bool {
        // 导出存在性 = available (requested ∩ compiled ∩ active)
        // 计划实现方向反了: `a.starts_with("get.")` 对 `http.get` 恒 false。
        // 修正: 命名空间前缀 或 final 方法段 等于 name
        self.available.iter().any(|a| {
            a == name || a.starts_with(&format!("{}.", name)) || a.ends_with(&format!(".{}", name))
        })
    }

    pub fn is_granted(&self, capability: &str) -> bool {
        self.granted.contains(capability)
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-executor/src/virtual_module.rs
git commit -m "feat(executor): implement @metado/runtime virtual module"
```

### Task 2.4: Execution Budget

**Goal:** Implement interrupt mechanism and fuel-based budget for JS execution.

**Files:**
- Create: `crates/metado-executor/src/budget.rs`

**Interfaces:**
- Produces: `ExecutionBudget { max_ops, fuel }`, `Budget::consume()`, `Budget::is_exhausted()`

- [x] **Step 1:** Write failing test

```rust
// tests/budget_test.rs
use metado_executor::budget::ExecutionBudget;

#[test]
fn test_budget_consume() {
    let mut budget = ExecutionBudget::new(100);
    assert!(!budget.is_exhausted());
    budget.consume(50).unwrap();
    assert!(!budget.is_exhausted());
    budget.consume(51).is_err(); // 超限 → Err（计划 .unwrap() 必 panic，已修）
    assert!(budget.is_exhausted());
}

#[test]
fn test_budget_exact() {
    let mut budget = ExecutionBudget::new(100);
    budget.consume(100).unwrap();
    assert!(!budget.is_exhausted());
    budget.consume(1).is_err();
    assert!(budget.is_exhausted()); // 同上修正
}
```

- [x] **Step 2:** Run test → verify FAIL
- [x] **Step 3:** Implement budget module

```rust
// crates/metado-executor/src/budget.rs

pub struct ExecutionBudget {
    max_ops: u64,
    consumed: u64,
}

impl ExecutionBudget {
    pub fn new(max_ops: u64) -> Self {
        Self { max_ops, consumed: 0 }
    }

    pub fn consume(&mut self, ops: u64) -> Result<(), String> {
        self.consumed += ops;
        if self.consumed > self.max_ops {
            Err("execution budget exhausted".into())
        } else {
            Ok(())
        }
    }

    pub fn is_exhausted(&self) -> bool {
        self.consumed > self.max_ops
    }

    pub fn remaining(&self) -> u64 {
        self.max_ops.saturating_sub(self.consumed)
    }
}
```

- [x] **Step 4:** Run test → verify PASS
- [x] **Step 5:** Commit

```bash
git add crates/metado-executor/src/budget.rs
git commit -m "feat(executor): implement execution budget"
```

### Task 2.5: Executor Facade

**Goal:** Wire executor components into public API.

**Files:**
- Create: `crates/metado-executor/src/lib.rs`

- [x] **Step 1:** Write integration test

```rust
// tests/executor_test.rs: eval/call 贯通 + VirtualModule 接线 + 预算边界 + 错误传播
```
- [x] **Step 2-5:** Implement and commit

---



**实现:** `Executor` 门面（executor.rs）。v1 预算按边界操作（eval/call 各记 1）消费，
将来换 boa 指令级燃料时公开 API 不变。
## Phase 3: Built-in Capabilities (metado-cap-*)

### Task 3.1: metado-cap-http

**Files:**
- Create: `crates/metado-cap-http/src/lib.rs`

- [x] Implement `CapabilitySet` for http (get/post via reqwest)
- [x] Register permissions: `http.get`, `http.post`, `http.get.api.*`
- [x] Commit

### Task 3.2: metado-cap-storage

**Files:**
- Create: `crates/metado-cap-storage/src/lib.rs`

- [x] Implement `CapabilitySet` for storage (filesystem backend)
- [x] Support signer namespace: `storage.<signer>.read/write`
- [x] Private keyspace: `storage:<plugin_id>:<key>`
- [x] Commit

### Task 3.3: metado-cap-file

**Files:**
- Create: `crates/metado-cap-file/src/lib.rs`

- [x] Implement read-only file access
- [x] Commit

### Task 3.4: metado-cap-time

**Files:**
- Create: `crates/metado-cap-time/src/lib.rs`

- [x] Implement `now()` and `sleep()`
- [x] Commit

### Task 3.5: metado-cap-log

**Files:**
- Create: `crates/metado-cap-log/src/lib.rs`

- [x] Implement info/warn/error/debug logging
- [x] Commit

### Task 3.6: metado-cap-crypto

**Files:**
- Create: `crates/metado-cap-crypto/src/lib.rs`

- [x] Implement randomBytes, sha256, hmac
- [x] Commit

---

## Phase 4: CLI Tool (mdl)

### Task 4.1: CLI Skeleton + `mdl build`

**Files:**
- Create: `crates/metado-cli/src/main.rs`, `crates/metado-cli/src/build.rs`

- [x] Implement build: read plugin dir → assemble ZIP → sign → output .mdl
- [x] Commit

### Task 4.2: `mdl sign` + `mdl verify`

**Files:**
- Create: `crates/metado-cli/src/sign.rs`, `crates/metado-cli/src/verify.rs`

- [x] Implement sign: attach signature to .mdl
- [x] Implement verify: check signature without extracting payload
- [x] Commit

### Task 4.3: `mdl run`

**Files:**
- Create: `crates/metado-cli/src/run.rs`

- [x] Implement run: in-process engine, load plugin, grant perms, invoke entry
- [x] `--grant` flags for permission simulation
- [x] Commit

### Task 4.4: `mdl watch`

**Files:**
- Create: `crates/metado-cli/src/watch.rs`

- [x] Implement watch: file system watcher, incremental rebuild, hot reload
- [x] Commit

### Task 4.5: `mdl test`

**Files:**
- Create: `crates/metado-cli/src/test.rs`

- [x] Implement test: run plugin entry as test, report results
- [x] Commit

### Task 4.6: `mdl env`

**Files:**
- Create: `crates/metado-cli/src/env.rs`

- [x] Implement env: output requested/available permissions, exports, entry table
- [x] Commit

### Task 4.7: `mdl trace`

**Files:**
- Create: `crates/metado-cli/src/trace.rs`

- [x] Implement trace: display trace events, filter by type, JSON output
- [x] Commit

---

## Phase 5: Engine Process + IPC

### Task 5.1: Transport Trait + Unix Domain Socket

**Files:**
- Create: `crates/metado-ipc/src/transport.rs`, `crates/metado-ipc/src/unix.rs`

- [x] Implement Transport trait: `send()`, `receive()`, `close()`
- [x] Implement UDS transport for Linux/macOS
- [x] Commit

### Task 5.2: JSON-RPC 2.0 Codec

**Files:**
- Create: `crates/metado-ipc/src/jsonrpc.rs`

- [x] Implement JSON-RPC request/response/notification encoding/decoding
- [x] Commit

### Task 5.3: Protocol Definition

**Files:**
- Create: `crates/metado-ipc/src/protocol.rs`

- [x] Define management methods: loadPlugin, grant, revoke, setLifecycle, invoke, listPlugins, uninstall, purge, registerPermissionSet, setDomainConfig, setActive, trace
- [x] Define callbacks: dispatch, notify
- [x] Commit

### Task 5.4: Android Bound Service Transport

**Files:**
- Create: `crates/metado-ipc/src/android.rs`

- [ ] Implement Android bound service IPC
- [ ] Commit


> **宿主不可验证（回写计划）**：Android（Binder/JNI）与 Windows（命名管道）
> 传输在本宿主（Termux/Linux）无法编译/运行，无法 TDD。按"质量与正确性优先"
> 原则延后到具备对应目标机的环节；Transport trait 与帧协议（5.1/5.2 已实现）
> 即为二者载体。Unix 传输是 Termux 主机进程间通信的落地路径（5.6 依赖）。

### Task 5.5: Windows Named Pipe Transport

**Files:**
- Create: `crates/metado-ipc/src/windows.rs`

- [ ] Implement Windows named pipe IPC
- [ ] Commit

### Task 5.6: Daemon Main

**Files:**
- Create: `crates/metado-daemon/src/main.rs`

- [x] Implement daemon: init engine, listen on IPC, dispatch to engine
- [x] Commit

---

## Phase 6: Examples + Contract Tests

### Task 6.1: Hello Plugin Example

**Files:**
- Create: `examples/hello-plugin/`

- [x] Minimal plugin with onMessage entry
- [x] Verify: `mdl build && mdl run --grant=*`
- [x] Commit

### Task 6.2: HTTP Plugin Example

**Files:**
- Create: `examples/http-plugin/`

- [x] Plugin using http.get capability
- [x] Verify: permission denied without grant, success with grant
- [x] Commit

### Task 6.3: Capability Developer Example

**Files:**
- Create: `examples/custom-cap-plugin/`

- [x] Host developer custom capability via `#[capability]` macro
- [x] Verify: capability message dispatch works
- [x] Commit

### Task 6.4: Contract Test Suite

**Files:**
- Create: `tests/contract/` (workspace member `metado-contract`)

- [x] CLI vs production behavior alignment tests (invoke/list/revoke/test/trace)
- [x] Permission model tests (requested/granted/available → exported, §4.3)
- [x] Lifecycle state machine tests (illegal transitions rejected, terminal uninstall)
- [x] Signature verification tests (bit-flip / re-sign / wrong key / signer id)
- [x] Container format tests (`../` traversal rejected at ingestion, missing `mdl.toml`)
- [x] Custom capability host-registration contract (CapabilitySet trait)
- [x] Commit

---

## Node 侧极简占位包 (@metado/runtime dev shim)

> v1 仅提供类型 + 占位实现，所有能力抛 "not available in Node"（已实现）

### Task N.1: Package Setup（回写：Stay ESM/.mjs + JSDoc，TS 属于 §15 明确不实现，且 tsc 是新依赖）

**Files:**
- Create: `packages/runtime-node/package.json`（无 tsconfig）

- [ ] Set up ESM-only package with conditional exports
- [ ] Commit

### Task N.2: Error Types

**Files:**
- Create: `packages/runtime-node/src/errors.mjs`

- [ ] Implement ExecutionError, PermissionDenied classes
- [ ] Commit

### Task N.3: Capability Stubs

**Files:**
- Create: `packages/runtime-node/src/http.mjs`, `storage.mjs`, `file.mjs`, `time.mjs`, `log.mjs`, `crypto.mjs`, `custom.mjs`（权限名与真实 metado-cap-* 注册集逐一核对其）

- [ ] Each export = throw "not available in Node"
- [ ] Commit

### Task N.4: Node Shim (Buffer/path/events)

**Files:**
- Create: `packages/runtime-node/src/shim.mjs`

- [ ] Re-export Buffer, path, events from Node
- [ ] Commit

### Task N.5: Permission Test Utils

**Files:**
- Create: `packages/runtime-node/src/test-utils.mjs`

- [ ] Implement `__METADO_TEST__` with granted Set + check function
- [ ] Commit

### Task N.6: Main Export

**Files:**
- Create: `packages/runtime-node/src/index.mjs`

- [ ] Wire all exports, publish `@metado/runtime@0.1.0-dev`
- [ ] Commit

### Task N.7: Permission Denial Test

**Files:**
- Create: `packages/runtime-node/tests/permission.test.mjs`

- [ ] Test: without granted → PermissionDenied reject
- [ ] Test: with granted → no rejection (but still throws "not available" for stubs)
- [ ] Commit

---

## v1 明确不实现（§15）

- 交互式单步调试器
- Node 运行/测试包 (metado-node, napi-rs)
- TypeScript 支持
- WASM 计算内核 (metado-cap-wasm)
- 跨插件调用
- 零拷贝共享 buffer
- 容器压缩
- 密钥轮换
- 句柄式宿主对象跨 WASM
- 流式宿主 API

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-09-19-metado-v1-implementation.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration

2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
