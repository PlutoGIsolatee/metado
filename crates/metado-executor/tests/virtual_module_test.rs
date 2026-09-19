//! Task 2.3: @metado/runtime virtual module tests

use metado_executor::VirtualModule;

#[test]
fn test_exports_filtered_by_available() {
    let vm = VirtualModule::build(
        vec!["http.get".into(), "storage.read".into()],
        vec!["http.get".into()],
    );
    // 导出存在性由 available 决定: http.get 的 final 段 "get" 导出
    assert!(vm.has_export("get"));
    assert!(vm.has_export("read"));
    // 命名空间段也可导出 (http.*)
    assert!(vm.has_export("http"));
    assert!(!vm.has_export("delete"));
    // 放行由 granted 决定
    assert!(vm.is_granted("http.get"));
    assert!(!vm.is_granted("storage.read"));
}

#[test]
fn test_missing_available_not_exported() {
    let vm = VirtualModule::build(vec!["http.get".into()], vec![]);
    // http.get 在 available 但不在 granted → 导出存在
    assert!(vm.has_export("get"));
}