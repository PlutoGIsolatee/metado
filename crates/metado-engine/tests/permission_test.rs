//! 权限匹配与 permission-set 展开测试（PermissionResolver 已于 2026-09-21 移除，
//! 其语义由 `permission_allows`/`grants_allow` 与 engine 权威 grant 取代）。

use metado_engine::permission::PermissionSet;

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