//! Task 1.6: Capability framework tests (§8.1)

use metado_engine::capability::{CapabilityMeta, CapabilityRegistry, CapabilitySet};

struct MockHttp;

impl CapabilitySet for MockHttp {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "http".into(),
            permissions: vec!["http.get".into(), "http.post".into()],
            exports: vec!["get".into(), "post".into()],
        }
    }
}

#[test]
fn test_register_and_lookup() {
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(MockHttp));

    let cap = registry.get("http").unwrap();
    assert_eq!(cap.meta().name, "http");
    assert!(cap.meta().permissions.contains(&"http.get".to_string()));
}

#[test]
fn test_available_permissions() {
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(MockHttp));

    let all_perms = registry.all_permissions();
    assert!(all_perms.contains(&"http.get".to_string()));
    assert!(all_perms.contains(&"http.post".to_string()));
}

#[test]
fn test_all_exports() {
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(MockHttp));

    let all_exports = registry.all_exports();
    assert!(all_exports.contains(&"get".to_string()));
    assert!(all_exports.contains(&"post".to_string()));
}