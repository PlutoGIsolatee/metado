//! Task 1.5: Permission resolver tests (§5)

use metado_engine::permission::{PermissionResolver, PermissionSet};

#[test]
fn test_requested_available_intersection() {
    let resolver = PermissionResolver::new(
        vec!["http.get".into(), "http.post".into(), "storage.read".into()],
        vec!["http.get".into(), "log.info".into()], // available = compiled ∩ active
    );
    // http.get 在 requested ∩ available → 可授
    // http.post 不在 available → 不可授
    // storage.read 在 requested 但不在 available → 不可授
    // log.info 不在 requested → 不可授
    assert!(resolver.can_grant("http.get"));
    assert!(!resolver.can_grant("http.post"));
    assert!(!resolver.can_grant("storage.read"));
    assert!(!resolver.can_grant("log.info"));
}

#[test]
fn test_grant_subset_of_requested() {
    let mut resolver = PermissionResolver::new(
        vec!["http.get".into(), "http.post".into()],
        vec!["http.get".into(), "http.post".into()],
    );
    resolver.grant(vec!["http.get".into()]);
    assert!(resolver.check("http.get").is_ok());
    assert!(resolver.check("http.post").is_err());
}

#[test]
fn test_signer_domain_binding() {
    let mut resolver = PermissionResolver::new(
        vec!["storage.<signer>.write".into()],
        vec!["storage.<signer>.write".into()],
    );
    resolver.bind_signer("abc123");
    resolver.grant(vec!["storage.abc123.write".into()]);
    assert!(resolver.check("storage.abc123.write").is_ok());
}

#[test]
fn test_resolve_signer_placeholder() {
    let mut resolver = PermissionResolver::new(Vec::new(), Vec::new());
    // 未绑定 signer 时原样返回
    assert_eq!(resolver.resolve_signer_placeholder("storage.<signer>.read"), "storage.<signer>.read");
    resolver.bind_signer("abc123");
    assert_eq!(
        resolver.resolve_signer_placeholder("storage.<signer>.read"),
        "storage.abc123.read"
    );
}

#[test]
fn test_revoke() {
    let mut resolver = PermissionResolver::new(
        vec!["http.get".into()],
        vec!["http.get".into()],
    );
    resolver.grant(vec!["http.get".into()]);
    assert!(resolver.check("http.get").is_ok());
    resolver.revoke(&["http.get".into()]);
    assert!(resolver.check("http.get").is_err());
}

#[test]
fn test_permission_set_expansion() {
    let mut ps = PermissionSet::new();
    ps.define("standard", vec!["http.get".into(), "log.info".into()]);
    let expanded = ps.expand(&["standard".into()]).unwrap();
    assert_eq!(expanded, vec!["http.get", "log.info"]);
}

#[test]
fn test_undefined_permission_set_fails() {
    let ps = PermissionSet::new();
    assert!(ps.expand(&["nonexistent".into()]).is_err());
}