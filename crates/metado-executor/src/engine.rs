use boa_engine::string::JsString;
use boa_engine::{Context, JsValue, Source};

use metado_engine::Value;

/// boa 0.22 桥接（Context::eval / JsObject::call 签名已按本地源码核对）。
pub struct JsEngine {
    context: Context,
}

impl JsEngine {
    pub fn new() -> Self {
        // C3 燃料（boa fuzz feature）：默认预算，避免 0 残留使一切立即失败；紧循环仍会被终结。
        let context = Context::builder()
            .instructions_remaining(crate::plugin_runtime::DEFAULT_INSTRUCTION_BUDGET)
            .build()
            .expect("build boa context");
        Self { context }
    }

    pub fn eval(&mut self, code: &str) -> Result<Value, String> {
        let result = self
            .context
            .eval(Source::from_bytes(code))
            .map_err(|e| format!("JS error: {:?}", e))?;
        Ok(js_to_value(&result))
    }

    pub fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        let func = self
            .context
            .eval(Source::from_bytes(name))
            .map_err(|e| format!("function not found: {:?}", e))?;
        let js_args: Vec<_> = args.iter().map(value_to_js).collect();
        let callable = func
            .as_callable()
            .ok_or_else(|| "not a function".to_string())?;
        // boa >= 0.20 (verified against 0.22.0 source): `call(&self, this, args, &mut Context)`
        let result = callable
            .call(&JsValue::undefined(), &js_args, &mut self.context)
            .map_err(|e| format!("call error: {:?}", e))?;
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
        // v1: 复杂对象（array/object/bytes）暂统一映射为 Null
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