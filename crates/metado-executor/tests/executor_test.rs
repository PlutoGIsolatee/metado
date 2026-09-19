//! Task 2.5: Executor facade integration tests

use metado_engine::Value;
use metado_executor::{Executor, VirtualModule};

#[test]
fn test_eval_and_call() {
    let mut ex = Executor::new();
    ex.eval("function add(a, b) { return a + b; }").unwrap();
    let out = ex.call("add", vec![Value::Number(2.0), Value::Number(3.0)]).unwrap();
    assert_eq!(out.as_f64(), Some(5.0));
}

#[test]
fn test_virtual_module_wired() {
    let mut ex = Executor::new();
    let vm = VirtualModule::build(
        vec!["http.get".into(), "storage.read".into()],
        vec!["http.get".into()],
    );
    ex.set_virtual_module(vm);
    assert!(ex.has_export("get"));
    assert!(ex.is_granted("http.get"));
    assert!(!ex.is_granted("storage.read"));
}

#[test]
fn test_budget_boundary() {
    let mut ex = Executor::new_with_budget(1); // 只允许一次边界操作
    ex.eval("var x = 1;").unwrap();
    assert!(ex.eval("var y = 2;").is_err()); // 预算耗尽
}

#[test]
fn test_call_error_propagates() {
    let mut ex = Executor::new();
    assert!(ex.call("missing", Vec::new()).is_err());
}