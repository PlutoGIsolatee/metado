//! Task 5.6: Daemon 库 —— 初始化引擎、监听 IPC、按管理面协议分发到引擎。
//! 生命周期/权限集登记走 `Engine`；真实插件执行经 `PluginRuntime`
//! （executor）驱动容器，与 CLI 一致。Unix serve 循环：逐连接读帧 →
//! handle_one → 回复，对端关闭帧即关闭连接。

use std::collections::HashMap;
use std::path::Path;

use serde_json::{json, Value};

use metado_cap_crypto::MetaCrypto;
use metado_cap_file::MetaFile;
use metado_cap_http::MetaHttp;
use metado_cap_log::MetaLog;
use metado_cap_storage::MetaStorage;
use metado_cap_time::MetaTime;
use metado_engine::{
    exported_namespaces, signer_id, CapabilityRegistry, Container, Engine, Manifest, SignedBundle,
    Value as EngineValue,
};
use metado_executor::PluginRuntime;
use metado_ipc::jsonrpc::{Request as RpcReq, Response};
use metado_ipc::protocol::{handle_one, ProtocolError};
use metado_ipc::transport::{IpcError, Transport};
use metado_ipc::unix::{UnixListener, UnixTransport};

/// 已加载插件（可执行记录）。
struct Record {
    name: String,
    signer: String,
    manifest: Manifest,
    container: Container,
    exported: Vec<String>,
    granted: Vec<String>,
    active: bool,
}

/// daemon 实例。
pub struct Daemon {
    engine: Engine,
    records: HashMap<String, Record>,
    version: u64,
}

impl Default for Daemon {
    fn default() -> Self {
        Self::new()
    }
}

const CAPS: [fn() -> Box<dyn metado_engine::CapabilitySet>; 6] = [
    || Box::new(MetaLog) as Box<dyn metado_engine::CapabilitySet>,
    || Box::new(MetaTime),
    || Box::new(MetaCrypto),
    || Box::new(MetaStorage),
    || Box::new(MetaFile),
    || Box::new(MetaHttp),
];

impl Daemon {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        for cap in CAPS {
            engine.register_capability(cap());
        }
        Self {
            engine,
            records: HashMap::new(),
            version: 0,
        }
    }

    /// 内置能力 permission 全集（available；与 CLI env 语义一致）。
    pub fn available_permissions(&self) -> Vec<String> {
        let mut reg = CapabilityRegistry::new();
        for cap in CAPS {
            reg.register(cap());
        }
        reg.all_permissions()
    }

    /// §4.3 导出命名空间 = 命名空间 ∈ available 命名空间的请求命名空间 ∪ {metado}。
    fn exported(&self, requested: &[String]) -> Vec<String> {
        exported_namespaces(requested, &self.available_permissions())
    }

    fn load_bundle(&mut self, bytes: &[u8], extra_permits: &[String]) -> Result<Value, String> {
        let bundle = SignedBundle::from_bytes(bytes)?;
        bundle.verify()?;
        let container = Container::from_bytes(bundle.payload())?;
        let manifest_raw = container.read_file("mdl.toml")?;
        let manifest_raw = String::from_utf8(manifest_raw)
            .map_err(|e| format!("manifest utf8: {}", e))?;
        let manifest =
            Manifest::from_toml(&manifest_raw).map_err(|e| format!("manifest: {}", e))?;

        let mut requested = manifest.permission.clone();
        requested.extend(extra_permits.iter().cloned());
        let exported = self.exported(&requested);

        self.engine.load_plugin(&bundle, requested.clone())?;
        self.engine.grant(&manifest.name, requested.clone())?;

        let sid = signer_id(&bundle.signer_pubkey());
        self.records.insert(
            manifest.name.clone(),
            Record {
                name: manifest.name.clone(),
                signer: sid,
                manifest: manifest.clone(),
                container,
                exported,
                granted: requested,
                active: false,
            },
        );
        Ok(json!({
            "plugin_id": manifest.name,
            "entries": manifest.entries.iter().map(|(n, d)| json!({"name": n, "export": d.export})).collect::<Vec<_>>(),
        }))
    }

    fn runtime(&self, plugin_id: &str) -> Result<(PluginRuntime, String), String> {
        let record = self
            .records
            .get(plugin_id)
            .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
        let files: HashMap<String, Vec<u8>> = record
            .container
            .files()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let files = Box::new(move |rel: &str| files.get(rel).cloned());
        let surface = self.available_permissions();
        let rt = PluginRuntime::new(&record.exported, &record.granted, &surface, files)?;
        let boot = record
            .manifest
            .entries
            .get("boot")
            .map(|d| d.export.clone())
            .unwrap_or_default();
        Ok((rt, boot))
    }

    /// 管理面方法分发（错误为 String）。
    pub fn method(&mut self, method: &str, params: &Value) -> Result<Value, String> {
        let str_arg = |k: &str| {
            params
                .get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("{}: missing string param {}", method, k))
        };
        match method {
            metado_ipc::protocol::LOAD_PLUGIN => {
                let file_hex = str_arg("file")?;
                let bytes = hex::decode(&file_hex).map_err(|e| format!("bad hex file: {}", e))?;
                let permits = params
                    .get("permits")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<String>>())
                    .unwrap_or_default();
                self.load_bundle(&bytes, &permits)
            }
            metado_ipc::protocol::LIST_PLUGINS => {
                let mut list: Vec<Value> = self
                    .records
                    .values()
                    .map(|r| {
                        let mut names: Vec<&String> = r.manifest.entries.keys().collect();
                        names.sort();
                        json!({
                            "id": r.name,
                            "name": r.name,
                            "signer": r.signer,
                            "active": r.active,
                            "granted": r.granted,
                            "entries": names,
                            "version": self.version,
                        })
                    })
                    .collect();
                list.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
                Ok(Value::Array(list))
            }
            metado_ipc::protocol::UNINSTALL => {
                let plugin_id = str_arg("plugin_id")?;
                if self.records.remove(&plugin_id).is_none() {
                    return Err(format!("plugin not found: {}", plugin_id));
                }
                self.version += 1;
                Ok(json!({ "uninstalled": plugin_id, "version": self.version }))
            }
            metado_ipc::protocol::PURGE => {
                let n = self.records.len();
                self.records.clear();
                self.version += 1;
                Ok(json!({ "purged": n, "version": self.version }))
            }
            metado_ipc::protocol::GRANT => {
                let plugin_id = str_arg("plugin_id")?;
                let record = self
                    .records
                    .get_mut(&plugin_id)
                    .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
                if let Some(perms) = params.get("permission").and_then(Value::as_array) {
                    record
                        .granted
                        .extend(perms.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<String>>());
                }
                let granted = record.granted.clone();
                self.engine.grant(&plugin_id, granted.clone())?;
                Ok(json!({ "granted": granted }))
            }
            metado_ipc::protocol::REVOKE => {
                let plugin_id = str_arg("plugin_id")?;
                if self.records.remove(&plugin_id).is_none() {
                    return Err(format!("plugin not found: {}", plugin_id));
                }
                self.version += 1;
                Ok(json!({ "revoked": plugin_id, "version": self.version }))
            }
            metado_ipc::protocol::SET_LIFECYCLE => {
                let plugin_id = str_arg("plugin_id")?;
                let state = str_arg("state")?;
                let record = self
                    .records
                    .get_mut(&plugin_id)
                    .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
                record.active = state != "unloaded";
                Ok(json!({ "state": state, "active": record.active }))
            }
            metado_ipc::protocol::SET_ACTIVE => {
                let plugin_id = str_arg("plugin_id")?;
                let active = params.get("active").and_then(Value::as_bool).unwrap_or(false);
                let record = self
                    .records
                    .get_mut(&plugin_id)
                    .ok_or_else(|| format!("plugin not found: {}", plugin_id))?;
                record.active = active;
                Ok(json!({ "active": active }))
            }
            metado_ipc::protocol::REGISTER_PERMISSION_SET => {
                let name = str_arg("name")?;
                let perms = params
                    .get("permissions")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect::<Vec<String>>())
                    .unwrap_or_default();
                self.engine.define_permission_set(&name, perms.clone());
                Ok(json!({ "name": name, "permissions": perms }))
            }
            metado_ipc::protocol::SET_DOMAIN_CONFIG => {
                let _domain = str_arg("domain")?;
                Ok(json!({ "set": true }))
            }
            metado_ipc::protocol::INVOKE => {
                let entry = str_arg("entry")?;
                let (plugin_id, entry_name) = entry
                    .split_once(':')
                    .ok_or_else(|| format!("entry must be plugin_id:entry, got {}", entry))?;
                let (mut rt, _boot) = self.runtime(plugin_id)?;
                let export = self
                    .records
                    .get(plugin_id)
                    .ok_or_else(|| format!("plugin not found: {}", plugin_id))?
                    .manifest
                    .entries
                    .get(entry_name)
                    .map(|d| d.export.clone())
                    .ok_or_else(|| format!("entry {} has no manifest definition", entry_name))?;
                rt.load("src/main.js").map_err(|e| format!("load: {}", e))?;
                let value_in = js_value_from_json(&params.get("args").cloned().unwrap_or(Value::Null));
                let out = rt.call_default(&export, value_in)?;
                Ok(engine_value_to_json(&out))
            }
            metado_ipc::protocol::TRACE => {
                let plugin_id = str_arg("plugin_id")?;
                let (mut rt, boot) = self.runtime(&plugin_id)?;
                if boot.is_empty() {
                    return Err("plugin has no boot entry".to_string());
                }
                let events: std::rc::Rc<std::cell::RefCell<Vec<Value>>> =
                    std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
                let sink_events = events.clone();
                rt.set_trace(Box::new(move |ev| {
                    sink_events
                        .borrow_mut()
                        .push(serde_json::to_value(ev).unwrap_or(Value::Null));
                }));
                rt.load("src/main.js").map_err(|e| format!("load: {}", e))?;
                let _ = rt.call_default(&boot, vec![]);
                let mut evs = events.borrow().clone();
                for e in evs.iter_mut() {
                    let kind = match e {
                        Value::Object(m) if m.contains_key("EntryStart") => "EntryStart",
                        Value::Object(m) if m.contains_key("EntryEnd") => "EntryEnd",
                        Value::Object(m) if m.contains_key("CapabilityCall") => "CapabilityCall",
                        Value::Object(m) if m.contains_key("PermissionCheck") => "PermissionCheck",
                        Value::Object(m) if m.contains_key("ValueFlow") => "ValueFlow",
                        _ => "Event",
                    };
                    e["kind"] = Value::String(kind.into());
                }
                Ok(Value::Array(evs))
            }
            other => Err(format!("unknown method {}", other)),
        }
    }

    /// JSON-RPC 请求 → 响应（通知→None）；错误映射为标准错误对象。
    pub fn handle_rpc(&mut self, req: &RpcReq) -> Option<Response> {
        match handle_one(req, &mut |m, p| self.method(m, p).map_err(ProtocolError::Dispatch)) {
            Ok(maybe) => maybe,
            Err(_) => None,
        }
    }

    /// Unix socket 服务循环：接受连接、逐帧分发、对端关闭帧即关连接。
    pub fn serve_unix(&mut self, path: &Path) -> Result<(), String> {
        let listener =
            UnixListener::bind(path).map_err(|e| format!("bind {}: {}", path.display(), e))?;
        loop {
            let (mut conn, _addr) = listener
                .accept()
                .map_err(|e| format!("accept: {}", e))?;
            // serve 需把会话驱动转发给 handle_rpc；receive 已在监听层读帧，这里复用消息循环
            self.serve_conn(&mut conn);
            conn.close();
        }
    }

    fn serve_conn(&mut self, conn: &mut UnixTransport) {
        loop {
            match conn.receive() {
                Ok(payload) => {
                    let text = match String::from_utf8(payload) {
                        Ok(t) => t,
                        Err(_) => continue,
                    };
                    let req = match RpcReq::from_json(&text) {
                        Ok(r) if !r.is_notification() => r,
                        _ => continue,
                    };
                    if let Some(resp) = self.handle_rpc(&req) {
                        if conn.send(&resp.to_json().to_string().into_bytes()).is_err() {
                            break;
                        }
                    }
                }
                Err(IpcError::Closed) => break,
                Err(e) => {
                    let _ = e;
                    break;
                }
            }
        }
    }
}

fn js_value_from_json(v: &Value) -> Vec<EngineValue> {
    match v {
        Value::Array(items) => items.iter().map(single_from_json).collect(),
        Value::Null => vec![],
        other => vec![single_from_json(other)],
    }
}

fn single_from_json(v: &Value) -> EngineValue {
    match v {
        Value::Null => EngineValue::Null,
        Value::Bool(b) => EngineValue::Bool(*b),
        Value::Number(n) => EngineValue::Number(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => EngineValue::String(s.clone()),
        Value::Array(items) => EngineValue::List(items.iter().map(single_from_json).collect()),
        Value::Object(m) => EngineValue::JsonMap(
            m.iter()
                .map(|(k, val)| (k.clone(), single_from_json(val)))
                .collect(),
        ),
    }
}

fn engine_value_to_json(v: &EngineValue) -> Value {
    match v {
        EngineValue::Null => Value::Null,
        EngineValue::Bool(b) => Value::Bool(*b),
        EngineValue::Number(n) => Value::from(*n),
        EngineValue::String(s) => Value::String(s.clone()),
        EngineValue::Bytes(b) => {
            let mut o = serde_json::Map::new();
            o.insert("$bytes".into(), Value::String(hex::encode(b)));
            Value::Object(o)
        }
        EngineValue::List(items) => Value::Array(items.iter().map(engine_value_to_json).collect()),
        EngineValue::JsonMap(m) => {
            let mut out = serde_json::Map::new();
            for (k, val) in m {
                out.insert(k.clone(), engine_value_to_json(val));
            }
            Value::Object(out)
        }
    }
}
