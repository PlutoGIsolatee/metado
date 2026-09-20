//! JSON-RPC 2.0 编解码（Task 5.2）：
//! - Request（含通知：无 id 不期望响应）
//! - Response（result 异或 error）
//! - 标准错误码常量
//! 协议方法名/载荷契约在 protocol.rs。

use serde_json::{json, Value};

use crate::transport::{IpcError, Result};

/// 请求：调用有 id、通知无 id。
#[derive(Debug, Clone)]
pub struct Request {
    id: Option<Value>,
    method: String,
    params: Value,
}

impl Request {
    pub fn named(method: &str) -> Self {
        Self {
            id: None,
            method: method.to_string(),
            params: Value::Null,
        }
    }

    pub fn with_id(mut self, id: impl Into<Value>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_params(mut self, params: Value) -> Self {
        self.params = params;
        self
    }

    /// 通知：无 id 字段。
    pub fn notification(method: &str, params: Value) -> Self {
        Self {
            id: None,
            method: method.to_string(),
            params,
        }
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn id(&self) -> Option<&Value> {
        self.id.as_ref()
    }

    pub fn params(&self) -> &Value {
        &self.params
    }

    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }

    pub fn to_json(&self) -> Value {
        let mut o = json!({"jsonrpc": "2.0", "method": self.method});
        if let Some(id) = &self.id {
            o["id"] = id.clone();
        }
        if self.params != Value::Null {
            o["params"] = self.params.clone();
        }
        o
    }

    pub fn from_json(s: &str) -> Result<Self> {
        let v: Value = serde_json::from_str(s)
            .map_err(|e| IpcError::Protocol(format!("bad request json: {}", e)))?;
        if v.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(IpcError::Protocol("missing jsonrpc=2.0".into()));
        }
        let method = v
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| IpcError::Protocol("missing method".into()))?
            .to_string();
        let id = v.get("id").cloned();
        let params = v.get("params").cloned().unwrap_or(Value::Null);
        Ok(Self { id, method, params })
    }
}

/// 错误对象（§11）。
#[derive(Debug, Clone)]
pub struct ErrorObject {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl ErrorObject {
    pub fn new(code: i64, message: &str, data: Option<Value>) -> Self {
        Self {
            code,
            message: message.to_string(),
            data,
        }
    }

    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error", None)
    }
    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request", None)
    }
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found", None)
    }
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params", None)
    }
    pub fn internal_error() -> Self {
        Self::new(-32603, "Internal error", None)
    }
}

/// 响应：`result`  异或 `error`。
#[derive(Debug, Clone)]
pub struct Response {
    pub id: Value,
    pub result: Option<Value>,
    pub error: Option<ErrorObject>,
}

impl Response {
    pub fn ok(id: impl Into<Value>, result: Value) -> Self {
        Self {
            id: id.into(),
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: impl Into<Value>, error: ErrorObject) -> Self {
        Self {
            id: id.into(),
            result: None,
            error: Some(error),
        }
    }

    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn to_json(&self) -> Value {
        let mut o = json!({"jsonrpc": "2.0", "id": self.id});
        if let Some(result) = &self.result {
            o["result"] = result.clone();
        }
        if let Some(err) = &self.error {
            o["error"] = json!({"code": err.code, "message": err.message});
            if let Some(data) = &err.data {
                o["error"]["data"] = data.clone();
            }
        }
        o
    }

    pub fn from_json(s: &str) -> Result<Self> {
        let v: Value = serde_json::from_str(s)
            .map_err(|e| IpcError::Protocol(format!("bad response json: {}", e)))?;
        if v.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(IpcError::Protocol("missing jsonrpc=2.0".into()));
        }
        let id = v
            .get("id")
            .cloned()
            .ok_or_else(|| IpcError::Protocol("missing id".into()))?;
        let has_result = v.get("result").is_some();
        let error = v.get("error").cloned();
        match (has_result, error) {
            (true, None) => Ok(Self {
                id,
                result: v.get("result").cloned(),
                error: None,
            }),
            (false, Some(e)) => Ok(Self {
                id,
                result: None,
                error: Some(ErrorObject {
                    code: e.get("code").and_then(Value::as_i64).unwrap_or(-32603),
                    message: e
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("error")
                        .to_string(),
                    data: e.get("data").cloned(),
                }),
            }),
            _ => Err(IpcError::Protocol("response must have result xor error".into())),
        }
    }
}

/// 调用糖：管理面 method 名 + 串行化载荷。
#[derive(Debug, Clone)]
pub struct RpcRequest {
    pub method: String,
    pub id: Value,
    pub params: Value,
}

impl RpcRequest {
    pub fn call(method: &str, id: impl Into<Value>, params: Value) -> Self {
        Self {
            method: method.to_string(),
            id: id.into(),
            params,
        }
    }
}