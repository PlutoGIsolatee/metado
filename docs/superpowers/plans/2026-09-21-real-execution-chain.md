# 真实执行链路（最小链路）Implementation Plan

> **⚠️ 归档/弃用通知**：此计划文档已废弃，仅供留档参考。后续执行以 `docs/superpowers/specs/2026-09-22-engine-replaceable-store-design.md` 为准，实现计划将另行编写。

# 真实执行链路（最小链路）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `@metado/runtime` 合成模块从形状 stub 升级为真实宿主 dispatch，使 `mdl run` 对 storage/log/time.now/crypto.randomBytes 端到端执行真实代码。

**Architecture:** executor 侧新增 `HostDispatch` trait 与唯一 `globalThis.__metadoDispatch` NativeFunction 桥；`PluginRuntime::new`（host=None）零回归，新增 `new_with_host`。cli 侧 `RealHost` 注入真实 cap 实现并在调用前二次权限裁决；`run_mdl` 经 `new_with_host` 接线。E2E 示例 + 测试验收架构行为。

**Tech Stack:** Rust 2021、boa_engine 0.22（`NativeFunction`/`FunctionObjectBuilder`/`JsPromise`）、现有 cap crates（log/time/crypto/storage）。

**Spec:** `docs/superpowers/specs/2026-09-21-real-execution-chain-design.md`

## Global Constraints

- 分支 `feat/v1-engine`；所有 git 提交带 `-c user.name="Metado Dev" -c user.email="dev@metado.local"`。
- build/test 一律 `--jobs 1`（Termux 内存受限）。
- 严格 TDD：先写失败测试（RED），再最小实现（GREEN），每步提交。
- 内置 API 语义保持基本：不改 cap 实现、不锁定返回值编码；值模型 v1 折叠规则不变（复杂对象→`Value::Null`）。
- 零回归基线：`cargo test --workspace --jobs 1` 当前 64 套件 / 214 passed / 0 failed / 0 warning。
- `HostDispatch` 错误用 `Result<Value, String>`；宿主二次裁决拒绝的消息以 `PermissionDenied:` 开头。
- 不新增第三方依赖。

---

## File Structure

- Create: `crates/metado-executor/src/host.rs` — `HostDispatch` trait（唯一职责：宿主能力分派接口）。
- Modify: `crates/metado-executor/src/lib.rs` — 导出 `host` 模块、`HostDispatch`、`DEFAULT_INSTRUCTION_BUDGET`。
- Modify: `crates/metado-executor/src/plugin_runtime.rs` — loader 增 `has_host`；`new_with_host`；注册 `__metadoDispatch`；形状工厂 host 分支。
- Modify: `crates/metado-executor/tests/plugin_runtime_test.rs` — 假 host 的桥测试。
- Create: `crates/metado-cli/src/host.rs` — `RealHost`（storage/log/time/crypto）。
- Modify: `crates/metado-cli/src/lib.rs` — `mod host; pub use host::RealHost;` + `run_mdl_with_storage`。
- Modify: `crates/metado-cli/src/run.rs` — 构造 `RealHost` 并接 `new_with_host`。
- Create: `crates/metado-cli/tests/host_test.rs` — `RealHost` 单元测试。
- Create: `crates/metado-cli/tests/run_real_test.rs` — 端到端。
- Create: `examples/storage-log/{mdl.toml,src/main.js}` — 示例。
- Modify: `docs/superpowers/LOG.md`、`docs/superpowers/2026-09-21-project-review-disposition.md` — 回写。

---

### Task 1: `HostDispatch` trait（executor）

**Files:**
- Create: `crates/metado-executor/src/host.rs`
- Modify: `crates/metado-executor/src/lib.rs`
- Test: `crates/metado-executor/tests/plugin_runtime_test.rs`（Task 2 中一并验证；本任务仅编译）

**Interfaces:**
- Produces: `pub trait HostDispatch { fn call(&mut self, ns: &str, method: &str, args: &[metado_engine::Value]) -> Result<metado_engine::Value, String>; }`

- [ ] **Step 1: 写 trait 文件**

Create `crates/metado-executor/src/host.rs`:

```rust
//! 宿主能力分派接口：executor 不持有内置实现，全部由宿主（cli/daemon）注入。

use metado_engine::Value;

/// 宿主能力分派入口。`Err(message)` 为能力失败；消息以 `PermissionDenied:` 开头表示
/// 宿主二次裁决拒绝。
pub trait HostDispatch {
    fn call(&mut self, ns: &str, method: &str, args: &[Value]) -> Result<Value, String>;
}
```

- [ ] **Step 2: 导出模块**

Modify `crates/metado-executor/src/lib.rs`，在 `pub mod engine;` 后加 `pub mod host;`，并在 re-export 区加：

```rust
pub use host::HostDispatch;
pub use plugin_runtime::{DEFAULT_INSTRUCTION_BUDGET, FilesFn, PluginRuntime, TraceHook};
```

（即把现有 `pub use plugin_runtime::{FilesFn, PluginRuntime, TraceHook};` 替换为含 `DEFAULT_INSTRUCTION_BUDGET` 的版本。）

- [ ] **Step 3: 编译验证**

Run: `cargo build -p metado-executor --jobs 1`
Expected: 编译通过（trait 未被使用不报错）。

- [ ] **Step 4: Commit**

```bash
git add crates/metado-executor/src/host.rs crates/metado-executor/src/lib.rs
git -c user.name="Metado Dev" -c user.email="dev@metado.local" commit -m "feat(executor): add HostDispatch trait"
```

---

### Task 2: `PluginRuntime` 桥接线（executor）

**Files:**
- Modify: `crates/metado-executor/src/plugin_runtime.rs`
- Test: `crates/metado-executor/tests/plugin_runtime_test.rs`

**Interfaces:**
- Consumes: `HostDispatch`（Task 1）。
- Produces: `PluginRuntime::new_with_host(exported: &[String], granted: &[String], surface: &[String], files: FilesFn, budget: usize, host: Option<Box<dyn HostDispatch>>) -> Result<Self, String>`；loader 字段 `has_host: bool`。

- [ ] **Step 1: 写失败测试（假 host）**

在 `crates/metado-executor/tests/plugin_runtime_test.rs` 顶部 `use` 后追加：

```rust
use std::cell::RefCell;
use std::rc::Rc;

use metado_executor::HostDispatch;

struct FakeHost {
    seen: Rc<RefCell<Vec<(String, String, Vec<Value>)>>>,
    reply: Result<Value, String>,
}

impl HostDispatch for FakeHost {
    fn call(&mut self, ns: &str, method: &str, args: &[Value]) -> Result<Value, String> {
        self.seen
            .borrow_mut()
            .push((ns.to_string(), method.to_string(), args.to_vec()));
        self.reply.clone()
    }
}

fn new_runtime_host(
    exported: &[String],
    granted: &[String],
    surface: Vec<String>,
    files: FilesFn,
    host: Box<dyn HostDispatch>,
) -> PluginRuntime {
    PluginRuntime::new_with_host(exported, granted, &surface, files, 50_000_000, Some(host)).unwrap()
}
```

在文件末尾追加 5 个测试：

```rust
#[test]
fn test_host_dispatch_sync_call() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let host = FakeHost { seen: seen.clone(), reply: Ok(Value::Null) };
    let files = stub(HashMap::from([(
        "src/main.js",
        "import { log } from '@metado/runtime';\n\
         export default { boot() { log.info('hi'); return 1; } };\n",
    )]));
    let mut rt = new_runtime_host(&[], &["log.info".into()], host_surface(), files, Box::new(host));
    rt.load("src/main.js").unwrap();
    assert_eq!(rt.call_default("boot", vec![]).unwrap().as_f64(), Some(1.0));
    let calls = seen.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "log");
    assert_eq!(calls[0].1, "info");
    assert_eq!(calls[0].2, vec![Value::String("hi".into())]);
}

#[test]
fn test_host_dispatch_promise_call_returns_value() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let host = FakeHost { seen: seen.clone(), reply: Ok(Value::String("v".into())) };
    let files = stub(HashMap::from([(
        "src/main.js",
        "import { storage } from '@metado/runtime';\n\
         export default { async boot() { return await storage.read('k'); } };\n",
    )]));
    let mut rt = new_runtime_host(&["storage".into()], &["storage.read".into()], host_surface(), files, Box::new(host));
    rt.load("src/main.js").unwrap();
    assert_eq!(rt.call_default("boot", vec![]).unwrap(), Value::String("v".into()));
    assert_eq!(seen.borrow()[0].0, "storage");
    assert_eq!(seen.borrow()[0].1, "read");
}

#[test]
fn test_host_dispatch_sync_error_throws() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let host = FakeHost { seen: seen.clone(), reply: Err("boom".into()) };
    let files = stub(HashMap::from([(
        "src/main.js",
        "import { log } from '@metado/runtime';\n\
         export default { boot() { log.info('x'); return 1; } };\n",
    )]));
    let mut rt = new_runtime_host(&[], &["log.info".into()], host_surface(), files, Box::new(host));
    rt.load("src/main.js").unwrap();
    let err = rt.call_default("boot", vec![]).unwrap_err();
    assert!(err.contains("boom"), "got: {}", err);
}

#[test]
fn test_host_dispatch_promise_error_rejects() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let host = FakeHost { seen: seen.clone(), reply: Err("PermissionDenied: storage.read".into()) };
    let files = stub(HashMap::from([(
        "src/main.js",
        "import { storage } from '@metado/runtime';\n\
         export default { async boot() { await storage.read('k'); return 1; } };\n",
    )]));
    let mut rt = new_runtime_host(&["storage".into()], &["storage.read".into()], host_surface(), files, Box::new(host));
    rt.load("src/main.js").unwrap();
    let err = rt.call_default("boot", vec![]).unwrap_err();
    assert!(err.contains("PermissionDenied"), "got: {}", err);
}

#[test]
fn test_dispatch_bridge_absent_without_host() {
    let files = stub(HashMap::from([(
        "src/main.js",
        "export default { boot() { return typeof globalThis.__metadoDispatch; } };\n",
    )]));
    let mut rt = new_runtime(&[], &[], host_surface(), files);
    rt.load("src/main.js").unwrap();
    assert_eq!(
        rt.call_default("boot", vec![]).unwrap(),
        Value::String("undefined".into())
    );
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p metado-executor --test plugin_runtime_test test_host_dispatch --jobs 1`
Expected: 编译失败（`new_with_host` 不存在）→ RED。

- [ ] **Step 3: 实现 loader 字段与桥**

在 `crates/metado-executor/src/plugin_runtime.rs`：

(a) `use` 区加：

```rust
use std::rc::Rc;

use boa_engine::object::FunctionObjectBuilder;
use boa_engine::object::builtins::JsPromise;
use boa_engine::native_function::NativeFunction;
use boa_engine::JsNativeError;

use crate::host::HostDispatch;
```

(b) `struct PluginModuleLoader` 加字段：

```rust
    /// 是否注册了宿主 dispatch 桥（决定形状工厂生成 host 方法还是 stub 方法）。
    has_host: bool,
```

(c) `runtime_module` 内生成 `src` 时把 host 分支织入。将现有 `let src = format!(...)` 中形状工厂主体替换为：

```rust
                    let use_host = self.has_host;
                    let src = format!(
                        "globalThis.__metadoShape = (() => {{\n\
                         const s = {spec_json};\n\
                         const useHost = {use_host};\n\
                         const deny = (cap) => {{ const e = new Error('permission denied: ' + cap); e.name = 'PermissionDenied'; return e; }};\n\
                         const mk = (ns, m, a, p) => {{\n\
                           const cap = ns + '.' + m;\n\
                           if (!a) return p ? function () {{ return Promise.reject(deny(cap)); }} : function () {{ throw deny(cap); }};\n\
                           if (useHost) return p ? function () {{ return Promise.resolve(globalThis.__metadoDispatch(ns, m, ...arguments)); }}\n\
                                               : function () {{ return globalThis.__metadoDispatch(ns, m, ...arguments); }};\n\
                           return p ? function () {{ return Promise.resolve(void 0); }} : function () {{ return void 0; }};\n\
                         }};\n\
                         const out = {{}};\n\
                         for (const ns in s) {{ const o = {{}}; for (const m in s[ns]) o[m] = mk(ns, m, s[ns][m].a, s[ns][m].p); out[ns] = o; }}\n\
                         return out;\n\
                         }})();"
                    );
```

(d) `new_with_instruction_budget` 改为委托新函数，并新增 `new_with_host`：

```rust
    pub fn new_with_instruction_budget(
        exported: &[String],
        granted: &[String],
        surface: &[String],
        files: FilesFn,
        budget: usize,
    ) -> Result<Self, String> {
        Self::new_with_host(exported, granted, surface, files, budget, None)
    }

    /// 同 `new`，但注入宿主 dispatch（`Some` 时放行方法经 `globalThis.__metadoDispatch` 真实调用）。
    pub fn new_with_host(
        exported: &[String],
        granted: &[String],
        surface: &[String],
        files: FilesFn,
        budget: usize,
        host: Option<Box<dyn HostDispatch>>,
    ) -> Result<Self, String> {
        let mut exported = exported.to_vec();
        if !exported.iter().any(|e| e == "metado") {
            exported.push("metado".into());
        }
        let ns_methods = build_ns_methods(surface);
        let spec_json = build_spec_json(&exported, granted, &ns_methods);
        let has_host = host.is_some();
        let host_rc: Option<Rc<RefCell<Box<dyn HostDispatch>>>> =
            host.map(|h| Rc::new(RefCell::new(h)));
        let loader = Rc::new(PluginModuleLoader {
            files,
            runtime_exports: exported,
            granted: granted.to_vec(),
            spec_json,
            ns_methods,
            has_host,
            trace: RefCell::new(None),
            cache: RefCell::new(HashMap::new()),
        });
        let mut context = Context::builder()
            .module_loader(loader.clone())
            .instructions_remaining(budget)
            .build()
            .map_err(|e| format!("runtime context: {}", e))?;
        if let Some(host_rc) = host_rc {
            let f = NativeFunction::from_copy_closure(move |_this, args, ctx| {
                dispatch_bridge(host_rc.clone(), args, ctx)
            });
            let func = FunctionObjectBuilder::new(context.realm(), f)
                .name("__metadoDispatch")
                .length(2)
                .build();
            context
                .global_object()
                .set(js_string!("__metadoDispatch"), func, false, &mut context)
                .map_err(|e| format!("register dispatch bridge: {}", e))?;
        }
        Ok(Self {
            context,
            loader,
            entry: None,
        })
    }
```

(e) 在 `impl PluginRuntime` 之外（文件底部）加桥实现：

```rust
/// `globalThis.__metadoDispatch(ns, method, ...args)`：唯一宿主桥。
/// 同步形状直接返回/抛错；promise 形状返回已 settle 的 Promise。
fn dispatch_bridge(
    host: Rc<RefCell<Box<dyn HostDispatch>>>,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ns = args
        .first()
        .and_then(|v| v.as_string())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let method = args
        .get(1)
        .and_then(|v| v.as_string())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let rest: Vec<Value> = args.iter().skip(2).map(js_to_value).collect();
    let promise_style = is_promise_style(&ns, &method);
    let result = host.borrow_mut().call(&ns, &method, &rest);
    match result {
        Ok(v) => {
            let js = value_to_js(&v);
            if promise_style {
                Ok(JsPromise::resolve(js, context)?.into())
            } else {
                Ok(js)
            }
        }
        Err(message) => {
            let err = JsNativeError::error().with_message(message);
            if promise_style {
                Ok(JsPromise::reject(err, context)?.into())
            } else {
                Err(err.into())
            }
        }
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p metado-executor --test plugin_runtime_test --jobs 1`
Expected: 全部 PASS（含既有 20 + 新增 5）。

- [ ] **Step 5: Commit**

```bash
git add crates/metado-executor/src/plugin_runtime.rs crates/metado-executor/tests/plugin_runtime_test.rs
git -c user.name="Metado Dev" -c user.email="dev@metado.local" commit -m "feat(executor): wire real host dispatch bridge (new_with_host)"
```

---

### Task 3: `RealHost`（cli 宿主侧实现）

**Files:**
- Create: `crates/metado-cli/src/host.rs`
- Modify: `crates/metado-cli/src/lib.rs`
- Test: `crates/metado-cli/tests/host_test.rs`

**Interfaces:**
- Consumes: `HostDispatch`（Task 1）、`metado_cap_storage::Storage`、`metado_cap_crypto::random_bytes`、`metado_engine::{Value, grants_allow}`。
- Produces: `RealHost::new(storage_dir: std::path::PathBuf, granted: Vec<String>) -> Result<Self, String>`，实现 `HostDispatch`。

- [ ] **Step 1: 写失败测试**

Create `crates/metado-cli/tests/host_test.rs`:

```rust
//! RealHost 单元测试（Task 3）：storage 回环、二次裁决拒绝、sync 直通。

use std::path::PathBuf;

use metado_cli::RealHost;
use metado_engine::Value;
use metado_executor::HostDispatch;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_dir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!("{}-{}-{}", tag, std::process::id(), n));
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn host(granted: &[&str]) -> RealHost {
    RealHost::new(temp_dir("metado-host"), granted.iter().map(|s| s.to_string()).collect()).unwrap()
}

#[test]
fn test_storage_round_trip() {
    let mut h = host(&["storage.read", "storage.write"]);
    h.call("storage", "write", &[Value::String("k".into()), Value::String("v".into())]).unwrap();
    let out = h.call("storage", "read", &[Value::String("k".into())]).unwrap();
    assert_eq!(out, Value::String("v".into()));
}

#[test]
fn test_storage_denied_by_secondary_check() {
    let mut h = host(&[]);
    let err = h.call("storage", "write", &[Value::String("k".into()), Value::String("v".into())]).unwrap_err();
    assert!(err.starts_with("PermissionDenied:"), "got: {}", err);
}

#[test]
fn test_time_now_returns_number() {
    let mut h = host(&["time.now"]);
    let out = h.call("time", "now", &[]).unwrap();
    assert!(matches!(out, Value::Number(n) if n > 0.0));
}

#[test]
fn test_random_bytes_sync_ok() {
    let mut h = host(&["crypto.randomBytes"]);
    let out = h.call("crypto", "randomBytes", &[Value::Number(4.0)]).unwrap();
    assert_eq!(out, Value::Null); // 值模型 v1 折叠，架构只验调用成功
}

#[test]
fn test_unimplemented_cap_reports() {
    let mut h = host(&["http.get"]);
    let err = h.call("http", "get", &[Value::String("http://x".into())]).unwrap_err();
    assert!(err.contains("not implemented (v1)"), "got: {}", err);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p metado-cli --test host_test --jobs 1`
Expected: 编译失败（`RealHost` 不存在）→ RED。

- [ ] **Step 3: 实现 RealHost**

Create `crates/metado-cli/src/host.rs`:

```rust
//! 宿主侧能力实现（最小链路）：storage / log / time.now / crypto.randomBytes 真实执行，
//! 其余能力返回「not implemented (v1)」。每次调用前对 granted 二次裁决（纵深防御）。

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use metado_engine::{grants_allow, Value};
use metado_executor::HostDispatch;

pub struct RealHost {
    storage: metado_cap_storage::Storage,
    granted: Vec<String>,
}

impl RealHost {
    pub fn new(storage_dir: PathBuf, granted: Vec<String>) -> Result<Self, String> {
        let dir = storage_dir.to_string_lossy().to_string();
        let storage = metado_cap_storage::Storage::new(&dir)?;
        Ok(Self { storage, granted })
    }

    fn ensure(&self, perm: &str) -> Result<(), String> {
        if grants_allow(&self.granted, perm) {
            Ok(())
        } else {
            Err(format!("PermissionDenied: {}", perm))
        }
    }
}

fn arg_string(args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

impl HostDispatch for RealHost {
    fn call(&mut self, ns: &str, method: &str, args: &[Value]) -> Result<Value, String> {
        match (ns, method) {
            ("storage", "read") => {
                self.ensure("storage.read")?;
                let key = arg_string(args, 0);
                let bytes = self.storage.read(&key)?;
                Ok(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
            }
            ("storage", "write") => {
                self.ensure("storage.write")?;
                let key = arg_string(args, 0);
                let value = arg_string(args, 1);
                self.storage.write(&key, value.as_bytes())?;
                Ok(Value::Null)
            }
            ("log", level @ ("info" | "warn" | "error" | "debug")) => {
                self.ensure(&format!("log.{}", level))?;
                let msg = arg_string(args, 0);
                if level == "error" {
                    eprintln!("[{}] {}", level, msg);
                } else {
                    println!("[{}] {}", level, msg);
                }
                Ok(Value::Null)
            }
            ("time", "now") => {
                self.ensure("time.now")?;
                let ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                Ok(Value::Number(ms as f64))
            }
            ("crypto", "randomBytes") => {
                self.ensure("crypto.randomBytes")?;
                let n = match args.first() {
                    Some(Value::Number(n)) if *n >= 0.0 => *n as usize,
                    _ => return Err("crypto.randomBytes: invalid length".into()),
                };
                let _ = metado_cap_crypto::random_bytes(n);
                Ok(Value::Null)
            }
            _ => Err(format!("not implemented (v1): {}.{}", ns, method)),
        }
    }
}
```

- [ ] **Step 4: 导出**

Modify `crates/metado-cli/src/lib.rs`：在 `mod env;` 附近加 `mod host;`，并在 re-export 区加：

```rust
pub use host::RealHost;
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p metado-cli --test host_test --jobs 1`
Expected: 5 passed。

- [ ] **Step 6: Commit**

```bash
git add crates/metado-cli/src/host.rs crates/metado-cli/src/lib.rs crates/metado-cli/tests/host_test.rs
git -c user.name="Metado Dev" -c user.email="dev@metado.local" commit -m "feat(cli): RealHost real capability dispatch (storage/log/time/crypto)"
```

---

### Task 4: `run_mdl` 接线 + E2E

**Files:**
- Modify: `crates/metado-cli/src/run.rs`
- Modify: `crates/metado-cli/src/lib.rs`
- Create: `examples/storage-log/mdl.toml`
- Create: `examples/storage-log/src/main.js`
- Test: `crates/metado-cli/tests/run_real_test.rs`

**Interfaces:**
- Consumes: `RealHost`（Task 3）、`PluginRuntime::new_with_host`（Task 2）、`DEFAULT_INSTRUCTION_BUDGET`（Task 1）。
- Produces: `run_mdl_with_storage(bytes: &[u8], extra_grant: &[String], storage_dir: &std::path::Path) -> Result<RunOutcome, String>`；`run_mdl` 委托之（storage_dir 取 `METADO_STORAGE_DIR` 或 `./metado-storage`）。

- [ ] **Step 1: 写示例**

Create `examples/storage-log/mdl.toml`:

```toml
name = "storage-log"
version = "1.0.0"
permission = ["storage.read", "storage.write", "log.info", "time.now", "crypto.randomBytes"]

[entries.boot]
export = "boot"
```

Create `examples/storage-log/src/main.js`:

```js
import { storage, log, time, crypto } from '@metado/runtime';

export default {
  boot() {
    const payload = `${time.now()}`;
    storage.write('k', payload);
    const v = storage.read('k');
    log.info(v);
    crypto.randomBytes(4);
    return v;
  },
};
```

- [ ] **Step 2: 写失败测试**

Create `crates/metado-cli/tests/run_real_test.rs`:

```rust
//! Task 4: mdl run 真实执行链路端到端（storage 回环 / 拒绝 / sync 直通）。

use metado_cli::{build_plugin, run_mdl_with_storage};
use metado_engine::{KeyPair, Value};

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!("{}-{}-{}", tag, std::process::id(), n));
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn build_plugin_dir(manifest: &str, main_js: &str) -> Vec<u8> {
    let root = temp_dir("metado-real-src");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("mdl.toml"), manifest).unwrap();
    std::fs::write(root.join("src/main.js"), main_js).unwrap();
    build_plugin(&root, &KeyPair::generate()).unwrap()
}

const MANIFEST: &str = "name = \"real\"\nversion = \"1.0.0\"\n\
permission = [\"storage.read\", \"storage.write\", \"log.info\", \"time.now\", \"crypto.randomBytes\"]\n\
[entries.boot]\nexport = \"boot\"\n";

#[test]
fn test_run_real_storage_round_trip() {
    let bytes = build_plugin_dir(
        MANIFEST,
        "import { storage, log, time, crypto } from '@metado/runtime';\n\
         export default { boot() {\n\
           const payload = `${time.now()}`;\n\
           storage.write('k', payload);\n\
           const v = storage.read('k');\n\
           log.info(v);\n\
           crypto.randomBytes(4);\n\
           return v;\n\
         } };\n",
    );
    let storage = temp_dir("metado-real-store");
    let outcome = run_mdl_with_storage(&bytes, &[], &storage).unwrap();
    assert!(outcome.invoked);
    match outcome.result {
        Value::String(s) => assert!(s.parse::<u128>().is_ok(), "expected millis, got: {}", s),
        other => panic!("expected string result, got {:?}", other),
    }
}

#[test]
fn test_run_real_denied_storage_write() {
    // 未请求 storage.write：调用期放行拒绝（异步形状 reject），await 后 boot 失败。
    let bytes = build_plugin_dir(
        "name = \"real-deny\"\nversion = \"1.0.0\"\n\
         permission = [\"storage.read\"]\n\
         [entries.boot]\nexport = \"boot\"\n",
        "import { storage } from '@metado/runtime';\n\
         export default { async boot() { await storage.write('k', 'v'); return 'no'; } };\n",
    );
    let storage = temp_dir("metado-real-deny");
    let err = run_mdl_with_storage(&bytes, &[], &storage).unwrap_err();
    assert!(
        err.to_lowercase().contains("permission denied"),
        "got: {}",
        err
    );
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p metado-cli --test run_real_test --jobs 1`
Expected: 编译失败（`run_mdl_with_storage` 不存在）→ RED。

- [ ] **Step 4: 实现接线**

Modify `crates/metado-cli/src/run.rs`：

(a) `use` 区加：

```rust
use std::path::{Path, PathBuf};

use crate::host::RealHost;
use metado_executor::DEFAULT_INSTRUCTION_BUDGET;
```

(b) `run_mdl` 改为委托：

```rust
pub fn run_mdl(bytes: &[u8], extra_grant: &[String]) -> Result<RunOutcome, String> {
    let dir = std::env::var("METADO_STORAGE_DIR")
        .unwrap_or_else(|_| "./metado-storage".to_string());
    run_mdl_with_storage(bytes, extra_grant, Path::new(&dir))
}

pub fn run_mdl_with_storage(
    bytes: &[u8],
    extra_grant: &[String],
    storage_dir: &Path,
) -> Result<RunOutcome, String> {
    // ...（原有 run_mdl 主体不变，仅末尾 PluginRuntime 构造改如下）...
```

(c) 把 `run_mdl` 原主体整段搬进 `run_mdl_with_storage`，并将末段：

```rust
    let mut rt = PluginRuntime::new(&exported, &granted, &available, files)?;
```

替换为：

```rust
    let host = RealHost::new(PathBuf::from(storage_dir), granted.clone())?;
    let mut rt = PluginRuntime::new_with_host(
        &exported,
        &granted,
        &available,
        files,
        DEFAULT_INSTRUCTION_BUDGET,
        Some(Box::new(host)),
    )?;
```

（`boot_export` 为 None 的早返回逻辑保留在 `run_mdl_with_storage` 内，早返回不构造 host 也可——构造 host 放在早返回之后。）

- [ ] **Step 5: 导出 `run_mdl_with_storage`**

Modify `crates/metado-cli/src/lib.rs` re-export：

```rust
pub use run::{run_mdl, run_mdl_with_storage, RunOutcome};
```

- [ ] **Step 6: 运行确认通过**

Run: `cargo test -p metado-cli --test run_real_test --jobs 1`
Expected: 2 passed。

- [ ] **Step 7: Commit**

```bash
git add crates/metado-cli/src/run.rs crates/metado-cli/src/lib.rs examples/storage-log crates/metado-cli/tests/run_real_test.rs
git -c user.name="Metado Dev" -c user.email="dev@metado.local" commit -m "feat(cli): wire RealHost into run_mdl + storage-log E2E example"
```

---

### Task 5: 全量回归 + 文档回写

**Files:**
- Modify: `docs/superpowers/LOG.md`
- Modify: `docs/superpowers/2026-09-21-project-review-disposition.md`

- [ ] **Step 1: 全量回归**

Run: `cargo test --workspace --jobs 1 > /data/data/com.termux/files/usr/tmp/opencode/regression3.log 2>&1; echo exit=$?`
Expected: exit=0。校验：`grep -cE "test result: ok" <log>` 全 ok，`grep -cE "failed:" <log>` = 0，`grep -cE "^warning" <log>` = 0。

- [ ] **Step 2: 回写 LOG**

在 `docs/superpowers/LOG.md` 末尾追加「会话八：真实执行链路（最小链路）」段：桥机制、`new_with_host`、`RealHost` 覆盖、E2E 结果、回归数字（含本轮新套件数/passed）。

- [ ] **Step 3: 回写台账**

在 `docs/superpowers/2026-09-21-project-review-disposition.md` §6 后追加 §7：记录「执行主体：最小真实链路已接线（storage/log/time/crypto），http/file/sleep/custom/daemon 待续」。

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/LOG.md docs/superpowers/2026-09-21-project-review-disposition.md
git -c user.name="Metado Dev" -c user.email="dev@metado.local" commit -m "docs: real execution chain (minimal) delivery + regression"
```

---

## Self-Review

**1. Spec coverage:**
- §2 单一 dispatch 桥 → Task 2（`__metadoDispatch` + 形状工厂 host 分支 + promise 包装）。
- §3 `HostDispatch` + `new_with_host` + back-compat → Task 1、Task 2。
- §4 `RealHost` 四能力 + 二次裁决 + not implemented → Task 3。
- §5 E2E 示例 + `run_real_test`（invoked/回环/sync 直通/拒绝）→ Task 4（sync 直通在 Task 3 `test_random_bytes_sync_ok`，E2E 正例含 randomBytes 调用）。
- §6 架构锁定项：内置 API 折叠不变（Task 3/4 未改 cap/值模型）；`storage.write` 串化（Task 3 `arg_string`）；log 前缀/流（Task 3）；缺 boot 早返回（Task 4 保留）；storage 目录 env（Task 4）。
- §7 测试策略 → Task 2（桥 5 测）、Task 3（RealHost 5 测）、Task 4（E2E 2 测）、Task 5（回归）。

**2. Placeholder scan:** 无 TBD/TODO；每步含可执行代码/命令。

**3. Type consistency:** `HostDispatch::call(&mut self, ns: &str, method: &str, args: &[Value]) -> Result<Value, String>` 在 Task 1 定义，Task 2 `dispatch_bridge` 与 Task 3 `impl` 一致；`new_with_host` 参数顺序 `(exported, granted, surface, files, budget, host)` 在 Task 2 定义、Task 4 调用一致；`RealHost::new(PathBuf, Vec<String>)` Task 3 定义、Task 4 调用一致；`run_mdl_with_storage(&[u8], &[String], &Path)` Task 4 定义并导出、测试一致。

**已知编译风险（实现时按 TDD 修正）：** boa 0.22 `JsFunction → JsValue` 与 `JsPromise → JsValue` 的 `Into` 实现名（必要时改 `JsValue::from(...)`）；`JsNativeError → JsError` 的 `into()`；`FunctionObjectBuilder`/`JsPromise` 的导入路径。均为导入/转换细节，不影响设计。
