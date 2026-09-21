//! 插件真实执行运行时（boa ESM）：容器文件树 + `@metado/runtime` 合成虚拟模块。
//! - 根模块从容器文件载入，相对 import（`./x.js`）经自定义 loader 解析容器内路径
//! - `@metado/runtime` = SyntheticModule，导出**命名空间对象**（方法函数面，§runtime-export-spec）
//!   - 导出存在性 = 命名空间级静态面（宿主裁定 exported，含恒在的 metado）
//!   - 调用放行 = 方法级动态面：未放行 → 异步形状 `Promise.reject(PermissionDenied)`，
//!     同步形状（log/time.now/crypto.randomBytes）同步 throw；放行 → v1 stub 值（Phase 5 接真体）
//! - 顶层 `default` 导出对象的方法经 `call_default` 调用；异步入口会 pump job 队列并读 Promise 状态

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::{self, Future};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use boa_engine::builtins::promise::PromiseState;
use boa_engine::module::{ModuleLoader, ModuleRequest, Referrer};
use boa_engine::object::builtins::JsPromise;
use boa_engine::{js_string, Context, JsNativeError, JsResult, JsString, JsValue, Module, Source};

use metado_engine::{grants_allow, TraceEvent, Value};

/// 容器文件访问器：入参为容器内相对路径（无前导 `/`）。
pub type FilesFn = Box<dyn Fn(&str) -> Option<Vec<u8>>>;

const RUNTIME_SPEC: &str = "@metado/runtime";

/// 同步形状（调用放行失败时同步 throw）：log 全方法、time.now、crypto.randomBytes。
fn is_promise_style(ns: &str, method: &str) -> bool {
    match (ns, method) {
        ("log", _) | ("time", "now") | ("crypto", "randomBytes") => false,
        _ => true,
    }
}

/// 恒放行的宿主命名空间：metado（host builtin 通用入口）与 custom（真实实现按 dispatch name 裁决）。
fn always_allowed(ns: &str) -> bool {
    ns == "metado" || ns == "custom"
}

/// 方法级放行裁决：非恒放行命名空间走 granted 模式匹配。
fn is_allowed(ns: &str, method: &str, granted: &[String]) -> bool {
    always_allowed(ns) || grants_allow(granted, &format!("{}.{}", ns, method))
}

/// 从 available 权限面推导命名空间 → 方法集合。custom 固定为 dispatch、metado 固定为 custom。
fn build_ns_methods(surface: &[String]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for perm in surface {
        let mut it = perm.split('.');
        let ns = it.next().unwrap_or("");
        let method = it
            .find(|seg| !seg.is_empty() && !seg.starts_with('<') && *seg != "*")
            .unwrap_or("");
        if !ns.is_empty() && !method.is_empty() {
            map.entry(ns.to_string()).or_default().push(method.to_string());
        }
    }
    map.insert("custom".into(), vec!["dispatch".into()]);
    map.insert("metado".into(), vec!["custom".into()]);
    for methods in map.values_mut() {
        methods.sort();
        methods.dedup();
    }
    map
}

struct PluginModuleLoader {
    files: FilesFn,
    /// `@metado/runtime` 实际导出的命名空间（含恒在的 metado）。
    runtime_exports: Vec<String>,
    /// 调用期授权模式集（方法级放行裁决输入）。
    granted: Vec<String>,
    /// 形状工厂 JSON 源（`ns.method → {a: allowed, p: promise-style}`），经 serde 编码防注入。
    spec_json: String,
    /// 命名空间 → 方法集合（工厂与轨迹使用）。
    ns_methods: HashMap<String, Vec<String>>,
    /// 轨迹钩子（§12.6）：记录 capability 调用与权限裁决；None = 不记录。
    trace: RefCell<Option<Rc<RefCell<dyn FnMut(TraceEvent)>>>>,
    cache: RefCell<HashMap<PathBuf, Module>>,
}

/// 轨迹钩子（§12.6）：每事件一次回调。
pub type TraceHook = Box<dyn FnMut(TraceEvent)>;

impl PluginModuleLoader {
    fn runtime_module(&self, context: &mut Context) -> JsResult<Module> {
        let path = PathBuf::from(RUNTIME_SPEC);
        if let Some(m) = self.cache.borrow().get(&path) {
            return Ok(m.clone());
        }
        let names: Vec<JsString> = self
            .runtime_exports
            .iter()
            .map(|n| JsString::from(n.as_str()))
            .collect();
        let export_names = self.runtime_exports.clone();
        let ns_methods = self.ns_methods.clone();
        let spec_json = self.spec_json.clone();
        let granted = self.granted.clone();
        let trace = self.trace.borrow().clone();

        // 形状工厂经 eval 建立（绕开 0.22 NativeFunction -> JsValue 的构造细节），
        // 权限/形状以 serde_json JSON 编码嵌入（注入安全）。闭包只捕获非 GC 类型。
        let initializer = unsafe {
            boa_engine::module::SyntheticModuleInitializer::from_closure(
                move |module, ctx| {
                    let src = format!(
                        "globalThis.__metadoShape = (() => {{\n\
                         const s = {spec_json};\n\
                         const deny = (cap) => {{ const e = new Error('permission denied: ' + cap); e.name = 'PermissionDenied'; return e; }};\n\
                         const mk = (a, p, cap) => p ? function () {{ return a ? Promise.resolve(void 0) : Promise.reject(deny(cap)); }}\n\
                         : function () {{ if (!a) throw deny(cap); return void 0; }};\n\
                         const out = {{}};\n\
                         for (const ns in s) {{ const o = {{}}; for (const m in s[ns]) o[m] = mk(s[ns][m].a, s[ns][m].p, ns + '.' + m); out[ns] = o; }}\n\
                         return out;\n\
                         }})();"
                    );
                    ctx.eval(Source::from_bytes(&src))?;
                    let shape = ctx
                        .global_object()
                        .get(js_string!("__metadoShape"), ctx)?;
                    let shape = shape
                        .as_object()
                        .ok_or_else(|| JsNativeError::error().with_message("shape init failed"))?;
                    for ns in &export_names {
                        let key = JsString::from(ns.as_str());
                        let obj = shape.get(key, ctx)?;
                        module.set_export(&JsString::from(ns.as_str()), obj)?;
                        for m in ns_methods.get(ns).cloned().unwrap_or_default() {
                            let capability = format!("{}.{}", ns, m);
                            let passed = is_allowed(ns, &m, &granted);
                            if let Some(t) = trace.as_ref() {
                                let mut t = t.borrow_mut();
                                t(TraceEvent::CapabilityCall {
                                    module: RUNTIME_SPEC.into(),
                                    line: 0,
                                    capability: capability.clone(),
                                    exported: export_names.clone(),
                                    granted: granted.clone(),
                                });
                                t(TraceEvent::PermissionCheck {
                                    capability: capability.clone(),
                                    passed,
                                });
                            }
                        }
                    }
                    Ok(())
                },
            )
        };
        let module = Module::synthetic(&names, initializer, Some(path.clone()), None, context);
        self.cache.borrow_mut().insert(path, module.clone());
        Ok(module)
    }

    /// 解析容器内路径：绝对（仓库内以 `/` 开头）或相对（相对 referrer 父目录），
    /// 折叠 `./`/`../` 分量并以容器内是否存在源码为判据。
    fn resolve(&self, specifier: &str, from: Option<&Path>) -> Option<PathBuf> {
        let candidate = match from {
            Some(from) => from.parent().unwrap_or(Path::new("/")).join(specifier),
            None => PathBuf::from(specifier),
        };
        let candidate = normalize_path(&candidate);
        let rel = candidate
            .to_string_lossy()
            .trim_start_matches('/')
            .to_string();
        if (self.files)(&rel).is_some() {
            Some(candidate)
        } else {
            None
        }
    }
}

/// 折平 `./`/`../` 分量的路径规范化（容器内路径无 `..` 逃逸面）。
fn normalize_path(path: &Path) -> PathBuf {
    let mut out: Vec<std::ffi::OsString> = Vec::new();
    let abs = path.is_absolute();
    for comp in path.components() {
        use std::path::Component::*;
        match comp {
            CurDir => {}
            ParentDir => {
                out.pop();
            }
            Prefix(_) | RootDir | Normal(_) => out.push(comp.as_os_str().to_os_string()),
        }
    }
    let mut joined = PathBuf::new();
    if abs {
        joined.push("/");
    }
    for c in out {
        joined.push(c);
    }
    joined
}

impl ModuleLoader for PluginModuleLoader {
    fn load_imported_module(
        self: Rc<Self>,
        referrer: Referrer,
        request: ModuleRequest,
        context: &RefCell<&mut Context>,
    ) -> impl Future<Output = JsResult<Module>> {
        let spec = request.specifier().to_std_string_escaped();
        let referrer_path = referrer.path().map(Path::to_path_buf);
        let result = (|| {
            let mut ctx = context.borrow_mut();
            if spec == RUNTIME_SPEC {
                return self.runtime_module(&mut ctx);
            }
            let path = self.resolve(&spec, referrer_path.as_deref()).ok_or_else(|| {
                JsNativeError::error().with_message(format!("module not found: {}", spec))
            })?;
            if let Some(m) = self.cache.borrow().get(&path) {
                return Ok(m.clone());
            }
            let rel = path
                .to_string_lossy()
                .trim_start_matches('/')
                .to_string();
            let code = (self.files)(&rel).ok_or_else(|| {
                JsNativeError::error().with_message(format!("no source for module: {}", rel))
            })?;
            let src = Source::from_bytes(&code).with_path(&path);
            let module = Module::parse(src, None, &mut ctx)?;
            self.cache.borrow_mut().insert(path, module.clone());
            Ok(module)
        })();
        future::ready(result)
    }
}

/// 插件运行时门面：load 根模块（link+evaluate），再 call default 导出方法。
pub struct PluginRuntime {
    context: Context,
    loader: Rc<PluginModuleLoader>,
    entry: Option<Module>,
}

/// 默认指令预算（C3 燃料）：紧循环/无限递归也会耗尽并报错，而非挂死 daemon。
pub const DEFAULT_INSTRUCTION_BUDGET: usize = 50_000_000;

impl PluginRuntime {
    /// `exported`：`@metado/runtime` 导出的命名空间（命名空间级静态面，恒含 metado）。
    /// `granted`：调用期放行的权限模式集（方法级裁决）。`surface`：available 权限全集（方法面来源）。
    pub fn new(exported: &[String], granted: &[String], surface: &[String], files: FilesFn) -> Result<Self, String> {
        Self::new_with_instruction_budget(exported, granted, surface, files, DEFAULT_INSTRUCTION_BUDGET)
    }

    /// 同 `new`，但指定逐指令预算（`budget` 耗尽 → 把指令节流为失败）。测试用较小值快速触发。
    pub fn new_with_instruction_budget(
        exported: &[String],
        granted: &[String],
        surface: &[String],
        files: FilesFn,
        budget: usize,
    ) -> Result<Self, String> {
        let mut exported = exported.to_vec();
        if !exported.iter().any(|e| e == "metado") {
            exported.push("metado".into());
        }
        let ns_methods = build_ns_methods(surface);
        let spec_json = build_spec_json(&exported, granted, &ns_methods);
        let loader = Rc::new(PluginModuleLoader {
            files,
            runtime_exports: exported,
            granted: granted.to_vec(),
            spec_json,
            ns_methods,
            trace: RefCell::new(None),
            cache: RefCell::new(HashMap::new()),
        });
        let context = Context::builder()
            .module_loader(loader.clone())
            .instructions_remaining(budget)
            .build()
            .map_err(|e| format!("runtime context: {}", e))?;
        Ok(Self {
            context,
            loader,
            entry: None,
        })
    }

    /// 挂接轨迹钩子（记录 capability 调用、权限裁决、入口起止、值流转）。
    pub fn set_trace(&mut self, hook: TraceHook) {
        let rc = Rc::new(RefCell::new(hook));
        *self.loader.trace.borrow_mut() = Some(rc);
    }

    /// 载入容器根模块（`src/main.js`），link + evaluate（含 `@metado/runtime` 解析）。
    pub fn load(&mut self, container_path: &str) -> Result<(), String> {
        let err = self.load_inner(container_path);
        match err {
            Err(msg) => self.map_fuel_error(msg).map(|_| ()),
            ok => ok,
        }
    }

    fn load_inner(&mut self, container_path: &str) -> Result<(), String> {
        let rel = container_path.trim_start_matches('/');
        let code = (self.loader.files)(rel)
            .ok_or_else(|| format!("entry module not found in container: {}", container_path))?;
        let path = PathBuf::from(format!("/{}", container_path.trim_start_matches('/')));
        let src = Source::from_bytes(&code).with_path(&path);
        let module = Module::parse(src, None, &mut self.context)
            .map_err(|e| format!("parse entry: {}", e))?;
        let promise = module.load_link_evaluate(&mut self.context);
        self.context
            .run_jobs()
            .map_err(|e| format!("module jobs: {}", e))?;
        if let PromiseState::Rejected(err) = promise.state() {
            let detail = err
                .as_string()
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|| "<non-string rejection>".into());
            return Err(format!("module evaluation rejected: {}", detail));
        }
        self.entry = Some(module);
        Ok(())
    }

    /// 指令预算耗尽（C3 燃料）→ 将原始错误转译为明确的预算耗尽错误。
    fn map_fuel_error(&self, msg: String) -> Result<(), String> {
        if self.context.instructions_remaining() == 0 {
            Err("instruction budget exhausted (C3 fuel)".into())
        } else {
            Err(msg)
        }
    }

    /// 调用根模块 default 导出的方法。
    /// 异步契约（§4.2/§9.1）：pump job 队列；入口返回 Promise → await 到 settled（reject → Err）。
    pub fn call_default(&mut self, method: &str, args: Vec<Value>) -> Result<Value, String> {
        match self.call_default_inner(method, args) {
            Err(_msg) if self.context.instructions_remaining() == 0 => {
                Err("instruction budget exhausted (C3 fuel)".into())
            }
            other => other,
        }
    }

    fn call_default_inner(&mut self, method: &str, args: Vec<Value>) -> Result<Value, String> {
        let module = self
            .entry
            .as_ref()
            .ok_or_else(|| "runtime not loaded".to_string())?;
        let ns = module.namespace(&mut self.context);
        let default_obj = ns
            .get(js_string!("default"), &mut self.context)
            .map_err(|e| format!("read default export: {}", e))?;
        let default_obj = default_obj.as_object().ok_or_else(|| {
            format!("entry must have a default object export with a '{}' method", method)
        })?;
        let callable = default_obj
            .get(js_string!(method), &mut self.context)
            .map_err(|e| format!("read method {}: {}", method, e))?;
        let callable = callable.as_callable().ok_or_else(|| {
            format!("'{}' is not a function on the default export", method)
        })?;
        let js_args: Vec<JsValue> = args.iter().map(value_to_js).collect();
        if let Some(t) = self.loader.trace.borrow().as_ref() {
            let mut t = t.borrow_mut();
            t(TraceEvent::EntryStart { entry: method.to_string() });
            t(TraceEvent::ValueFlow { direction: "in".into(), size_hint: js_args.len() });
        }
        let result = callable
            .call(&JsValue::from(default_obj), &js_args, &mut self.context)
            .map_err(|e| format!("call {}: {}", method, e))?;
        // 异步输入：pump job 队列，入口 Promise 读取真实状态
        self.context
            .run_jobs()
            .map_err(|e| format!("{} jobs: {}", method, e))?;
        let value = if result.is_promise() {
            let object = result
                .as_object()
                .ok_or_else(|| format!("{} returned promise without object", method))?;
            let promise = JsPromise::from_object(object)
                .map_err(|e| format!("{} promise wrap: {}", method, e))?;
            // await_blocking 泵 job 至 settled；reject 流为 Err。注：永悬 Promise 会挂起（C3 燃料/中断边界外）
            promise
                .await_blocking(&mut self.context)
                .map_err(|e| format!("{} rejected: {}", method, e))?
        } else {
            result
        };
        if let Some(t) = self.loader.trace.borrow().as_ref() {
            let mut t = t.borrow_mut();
            t(TraceEvent::ValueFlow { direction: "out".into(), size_hint: 1 });
            t(TraceEvent::EntryEnd { entry: method.to_string() });
        }
        Ok(js_to_value(&value))
    }
}

/// 形状工厂 JSON：`{ "<ns>": { "<method>": {"a": allowed, "p": promise-style} } }`。
fn build_spec_json(
    exported: &[String],
    granted: &[String],
    ns_methods: &HashMap<String, Vec<String>>,
) -> String {
    let mut spec = serde_json::Map::new();
    for ns in exported {
        let methods = ns_methods.get(ns).cloned().unwrap_or_default();
        let mut m = serde_json::Map::new();
        for method in methods {
            let mut entry = serde_json::Map::new();
            entry.insert("a".to_string(), is_allowed(ns, &method, granted).into());
            entry.insert("p".to_string(), is_promise_style(ns, &method).into());
            m.insert(method, serde_json::Value::Object(entry));
        }
        spec.insert(ns.clone(), serde_json::Value::Object(m));
    }
    serde_json::Value::Object(spec).to_string()
}

fn js_to_value(val: &JsValue) -> Value {
    if val.is_null_or_undefined() {
        Value::Null
    } else if let Some(b) = val.as_boolean() {
        Value::Bool(b)
    } else if let Some(n) = val.as_number() {
        Value::Number(n)
    } else if let Some(s) = val.as_string() {
        Value::String(s.to_std_string_escaped())
    } else {
        // 复杂对象（array/object/bytes）v1 统一折叠为 Null；完整映射 Phase 5
        Value::Null
    }
}

fn value_to_js(val: &Value) -> JsValue {
    match val {
        Value::Null => JsValue::null(),
        Value::Bool(b) => (*b).into(),
        Value::Number(n) => (*n).into(),
        Value::String(s) => JsValue::from(JsString::from(s.as_str())),
        _ => JsValue::null(),
    }
}