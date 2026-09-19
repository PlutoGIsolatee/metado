//! Task 1.1: Value model tests (§4.4)
//! 先写测试，验证失败后再实现。

use metado_engine::value::Value;

#[test]
fn test_null_value() {
    let v = Value::Null;
    assert!(!v.is_truthy());
}

#[test]
fn test_false_value() {
    let v = Value::Bool(false);
    assert!(!v.is_truthy());
}

#[test]
fn test_zero_number_falsy() {
    let v = Value::Number(0.0);
    assert!(!v.is_truthy());
}

#[test]
fn test_empty_string_falsy() {
    let v = Value::String("".into());
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