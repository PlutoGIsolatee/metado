//! 管理面协议（§11）：方法名即契约、载荷校验、请求/通知分发。
//! daemon（Task 5.6）以 `handle_one` 驱动：请求→响应（错误映射 ErrorObject），通知→无响应。
//! 回调面（dispatch/notify）由 client 侧对端发起，daemon 侧收发皆为此协议。

use serde_json::{json, Value};

use crate::jsonrpc::{ErrorObject, Request, Response};
use crate::transport::Result;

// --- 管理方法名（稳定字符串契约） ---
pub const LOAD_PLUGIN: &str = "loadPlugin";
pub const GRANT: &str = "grant";
pub const REVOKE: &str = "revoke";
pub const SET_LIFECYCLE: &str = "setLifecycle";
pub const INVOKE: &str = "invoke";
pub const LIST_PLUGINS: &str = "listPlugins";
pub const UNINSTALL: &str = "uninstall";
pub const PURGE: &str = "purge";
pub const REGISTER_PERMISSION_SET: &str = "registerPermissionSet";
pub const SET_DOMAIN_CONFIG: &str = "setDomainConfig";
pub const SET_ACTIVE: &str = "setActive";
pub const TRACE: &str = "trace";
// --- 回调面方法名 ---
pub const CB_DISPATCH: &str = "dispatch";
pub const CB_NOTIFY: &str = "notify";

#[derive(Debug, Clone)]
pub enum Params {
    LoadPlugin { file_hex: String, permits: Vec<String> },
    Invoke(InvokeParams),
    ListPlugins,
    Uninstall { plugin_id: String },
    Purge,
    SetActive { plugin_id: String, active: bool },
    Grant { plugin_id: String, permission: Vec<String> },
    Revoke { plugin_id: String },
    SetLifecycle { plugin_id: String, state: String },
    RegisterPermissionSet { name: String, permissions: Vec<String> },
    SetDomainConfig { domain: String, config: Value },
    Trace { plugin_id: String },
}

#[derive(Debug, Clone)]
pub struct InvokeParams {
    pub entry: String,
    pub args: Value,
}

impl Params {
    pub fn method_name(&self) -> &'static str {
        match self {
            Params::LoadPlugin { .. } => LOAD_PLUGIN,
            Params::Invoke(_) => INVOKE,
            Params::ListPlugins => LIST_PLUGINS,
            Params::Uninstall { .. } => UNINSTALL,
            Params::Purge => PURGE,
            Params::SetActive { .. } => SET_ACTIVE,
            Params::Grant { .. } => GRANT,
            Params::Revoke { .. } => REVOKE,
            Params::SetLifecycle { .. } => SET_LIFECYCLE,
            Params::RegisterPermissionSet { .. } => REGISTER_PERMISSION_SET,
            Params::SetDomainConfig { .. } => SET_DOMAIN_CONFIG,
            Params::Trace { .. } => TRACE,
        }
    }

    pub fn to_value(&self) -> Value {
        match self {
            Params::LoadPlugin { file_hex, permits } => {
                json!({ "file": file_hex, "permits": permits })
            }
            Params::Invoke(i) => json!({ "entry": i.entry, "args": i.args }),
            Params::ListPlugins => json!({}),
            Params::Uninstall { plugin_id } => json!({ "plugin_id": plugin_id }),
            Params::Purge => json!({}),
            Params::SetActive { plugin_id, active } => json!({ "plugin_id": plugin_id, "active": active }),
            Params::Grant { plugin_id, permission } => {
                json!({ "plugin_id": plugin_id, "permission": permission })
            }
            Params::Revoke { plugin_id } => json!({ "plugin_id": plugin_id }),
            Params::SetLifecycle { plugin_id, state } => {
                json!({ "plugin_id": plugin_id, "state": state })
            }
            Params::RegisterPermissionSet { name, permissions } => {
                json!({ "name": name, "permissions": permissions })
            }
            Params::SetDomainConfig { domain, config } => {
                json!({ "domain": domain, "config": config })
            }
            Params::Trace { plugin_id } => json!({ "plugin_id": plugin_id }),
        }
    }

    /// 反向（方法名、载荷签收），校验不全即 BadParams。
    pub fn from_method(method: &str, params: &Value) -> std::result::Result<Params, ProtocolError> {
        let need_str = |k: &str, v: &Value| {
            v.get(k)
                .and_then(Value::as_str)
                .ok_or_else(|| ProtocolError::BadParams(method.to_string()))
                .map(str::to_string)
        };
        use std::result::Result::*;
        match method {
            LOAD_PLUGIN => {
                let file_hex = need_str("file", params)?;
                let permits = params
                    .get("permits")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(Params::LoadPlugin { file_hex, permits })
            }
            INVOKE => Ok(Params::Invoke(InvokeParams {
                entry: need_str("entry", params)?,
                args: params.get("args").cloned().unwrap_or(Value::Null),
            })),
            LIST_PLUGINS => Ok(Params::ListPlugins),
            UNINSTALL => Ok(Params::Uninstall { plugin_id: need_str("plugin_id", params)? }),
            PURGE => Ok(Params::Purge),
            SET_ACTIVE => Ok(Params::SetActive {
                plugin_id: need_str("plugin_id", params)?,
                active: params.get("active").and_then(Value::as_bool).unwrap_or(false),
            }),
            GRANT => Ok(Params::Grant {
                plugin_id: need_str("plugin_id", params)?,
                permission: params
                    .get("permission")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default(),
            }),
            REVOKE => Ok(Params::Revoke { plugin_id: need_str("plugin_id", params)? }),
            SET_LIFECYCLE => Ok(Params::SetLifecycle {
                plugin_id: need_str("plugin_id", params)?,
                state: need_str("state", params)?,
            }),
            REGISTER_PERMISSION_SET => Ok(Params::RegisterPermissionSet {
                name: need_str("name", params)?,
                permissions: params
                    .get("permissions")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default(),
            }),
            SET_DOMAIN_CONFIG => Ok(Params::SetDomainConfig {
                domain: need_str("domain", params)?,
                config: params.get("config").cloned().unwrap_or(Value::Null),
            }),
            TRACE => Ok(Params::Trace { plugin_id: need_str("plugin_id", params)? }),
            other => Err(ProtocolError::UnknownMethod(other.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ProtocolError {
    UnknownMethod(String),
    BadParams(String),
    Dispatch(String),
}

impl ProtocolError {
    pub fn message(&self) -> String {
        match self {
            ProtocolError::UnknownMethod(m) => format!("unknown method {}", m),
            ProtocolError::BadParams(m) => format!("bad params for {}", m),
            ProtocolError::Dispatch(r) => format!("engine: {}", r),
        }
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for ProtocolError {}

impl From<ProtocolError> for ErrorObject {
    fn from(e: ProtocolError) -> Self {
        let (code, data) = match &e {
            ProtocolError::UnknownMethod(_) => (-32601, None),
            ProtocolError::BadParams(_) => (-32602, None),
            ProtocolError::Dispatch(_) => (-32603, None),
        };
        ErrorObject::new(code, &e.message(), data)
    }
}

/// Dispatch 单请求：请求→Some(Reply)；通知→None（不回复）。
/// 未知方法/参数错误 → 标准错误响应。
pub fn handle_one(
    req: &Request,
    handler: &mut dyn FnMut(&str, &Value) -> std::result::Result<Value, ProtocolError>,
) -> Result<Option<Response>> {
    if req.is_notification() {
        let _ = call_handler(req, handler);
        return Ok(None);
    }
    let result = call_handler(req, handler);
    let resp = match result {
        Ok(v) => Response::ok(req.id().cloned().unwrap_or(Value::Null), v),
        Err(e) => Response::error(req.id().cloned().unwrap_or(Value::Null), ErrorObject::from(e)),
    };
    Ok(Some(resp))
}

fn call_handler(
    req: &Request,
    handler: &mut dyn FnMut(&str, &Value) -> std::result::Result<Value, ProtocolError>,
) -> std::result::Result<Value, ProtocolError> {
    if req.method() == CB_DISPATCH || req.method() == CB_NOTIFY {
        return Err(ProtocolError::Dispatch(format!("callback {} not handled", req.method())));
    }
    let _ = Params::from_method(req.method(), req.params())?;
    handler(req.method(), req.params())
}

