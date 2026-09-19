//! Task 1.2: Error model tests (§9.2)

use metado_engine::error::{ExecutionError, ErrorKind};

#[test]
fn test_plugin_error() {
    let e = ExecutionError::new("onMessage", ErrorKind::PluginError, "JS threw");
    assert_eq!(e.entry, "onMessage");
    assert_eq!(e.kind, ErrorKind::PluginError);
    assert_eq!(e.message, "JS threw");
}

#[test]
fn test_permission_denied() {
    let e = ExecutionError::new("onMessage", ErrorKind::PermissionDenied, "http.get");
    assert_eq!(e.kind, ErrorKind::PermissionDenied);
}

#[test]
fn test_capability_error() {
    let e = ExecutionError::new("onMessage", ErrorKind::CapabilityError, "storage write failed");
    assert_eq!(e.kind, ErrorKind::CapabilityError);
}

#[test]
fn test_fault() {
    let e = ExecutionError::new("onMessage", ErrorKind::Fault, "fuel exhausted");
    assert_eq!(e.kind, ErrorKind::Fault);
}

#[test]
fn test_display() {
    let e = ExecutionError::new("boot", ErrorKind::PermissionDenied, "storage.read");
    let s = e.to_string();
    assert!(s.contains("PermissionDenied"));
    assert!(s.contains("boot"));
    assert!(s.contains("storage.read"));
}