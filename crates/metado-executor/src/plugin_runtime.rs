//! 插件真实执行运行时（boa ESM）：容器文件树 + `@metado/runtime` 合成虚拟模块。
//! - 根模块从容器文件载入，相对 import（`./x.js`）经自定义 loader 解析容器内路径
//! - `@metado/runtime` = SyntheticModule，导出名 = available 能力命名空间
//!   （§4.3：未请求的能力导出不存在；调用放行由 granted 决定 → v1 函数体为 stub，Phase 5 接真体）
//! - 顶层 `default` 导出对象的方法经 `call_default` 调用

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::{self, Future};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use boa_engine::builtins::promise::PromiseState;
use boa_engine::module::{ModuleLoader, ModuleRequest, Referrer};
use boa_engine::{js_string, Context, JsNativeError, JsResult, JsString, JsValue, Module, Source};

use metado_engine::{TraceEvent, Value};

/// 容器文件访问器：入参为容器内相对路径（无前导 `/`）。
pub type FilesFn = Box<dyn Fn(&str) -> Option<Vec<u8>>>;

const RUNTIME_SPEC: &str = "@metado/runtime";

/// 由 available 权限推导 `@metado/runtime` 导出名（能力命名空间集合）。
/// 永远追加 `metado`（通用调用入口，宿主内建）。
pub fn runtime_namespaces(available: &[String]) -> Vec<String> {
    let mut namespaces: Vec<String> = available
        .iter()
        .map(|a| a.split('.').next().unwrap_or("").to_string())
        .filter(|n| !n.is_empty())
        .collect();
    namespaces.push("metado".into());
    namespaces.sort();
    namespaces.dedup();
    namespaces
}

struct PluginModuleLoader {
    files: FilesFn,
    runtime_exports: Vec<String>,
    /// 轨迹钩子（§12.6）：记录 capability 调用与值流转；None = 不记录。
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
        let names: Vec<JsString> = self.runtime_exports.iter().map(|n| JsString::from(n.as_str())).collect();
        let export_names = self.runtime_exports.clone();
        let trace = self.trace.borrow().clone();
        // 初始化的 export 值：v1 为 stub 函数（返回 undefined；调用体在 Phase 5）。
        // 闭包只捕获 String/Rc（非 GC 可追踪类型），from_closure 安全。stub 经 eval 创建，
        // 绕开 0.22 NativeFunction -> JsValue 的构造细节。每个导出记录一条 CapabilityCall 轨迹。
        let initializer = unsafe {
            boa_engine::module::SyntheticModuleInitializer::from_closure(
                move |module, ctx| {
                    for name in &export_names {
                        if let Some(t) = trace.as_ref() {
                            (t.borrow_mut())(TraceEvent::CapabilityCall {
                                module: RUNTIME_SPEC.into(),
                                line: 0,
                                capability: name.clone(),
                                requested: export_names.clone(),
                                granted: export_names.clone(),
                            });
                        }
                        let stub = ctx.eval(Source::from_bytes("(function () {})"))?;
                        module.set_export(&JsString::from(name.as_str()), stub.clone())?;
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

impl PluginRuntime {
    /// `exported`：`@metado/runtime` 实际导出的命名空间（§4.3 由宿主裁定 = requested ∩ available，v1 恒含 metado）。
    /// 容器访问经 `files`；权限放行（granted 裁决）在能力函数体，Phase 5 接入。
    pub fn new(exported: &[String], files: FilesFn) -> Result<Self, String> {
        let loader = Rc::new(PluginModuleLoader {
            files,
            runtime_exports: exported.to_vec(),
            trace: RefCell::new(None),
            cache: RefCell::new(HashMap::new()),
        });
        let context = Context::builder()
            .module_loader(loader.clone())
            .build()
            .map_err(|e| format!("runtime context: {}", e))?;
        Ok(Self {
            context,
            loader,
            entry: None,
        })
    }

    /// 挂接轨迹钩子（记录 capability 调用、入口起止、值流转）。
    pub fn set_trace(&mut self, hook: TraceHook) {
        let rc = Rc::new(RefCell::new(hook));
        *self.loader.trace.borrow_mut() = Some(rc);
    }

    /// 载入容器根模块（`src/main.js`），link + evaluate（含 `@metado/runtime` 解析）。
    pub fn load(&mut self, container_path: &str) -> Result<(), String> {
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

    /// 调用根模块 default 导出的方法。
    pub fn call_default(&mut self, method: &str, args: Vec<Value>) -> Result<Value, String> {
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
        if let Some(t) = self.loader.trace.borrow().as_ref() {
            let mut t = t.borrow_mut();
            t(TraceEvent::ValueFlow { direction: "out".into(), size_hint: 1 });
            t(TraceEvent::EntryEnd { entry: method.to_string() });
        }
        Ok(js_to_value(&result))
    }
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