//! Task 2.1: boa JS engine integration tests

use metado_engine::Value;
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
    let result = engine
        .call("add", vec![Value::Number(1.0), Value::Number(2.0)])
        .unwrap();
    assert_eq!(result.as_f64(), Some(3.0));
}

#[test]
fn test_eval_error() {
    let mut engine = JsEngine::new();
    assert!(engine.eval("this is not js .").is_err());
}

#[test]
fn test_call_non_function() {
    let mut engine = JsEngine::new();
    engine.eval("var x = 42;").unwrap();
    assert!(engine.call("x", Vec::new()).is_err());
}