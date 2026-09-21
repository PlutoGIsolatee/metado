//! Task 6.4 契约：权限模型（requested / granted / available → exported）。

use metado_contract::common::bundle_bytes;
use metado_cli::{env_mdl, exported_namespaces, registry_permissions, run_mdl, test_mdl};

#[test]
fn contract_exported_is_requested_intersect_available_plus_metado() {
    // §4.3：导出命名空间 = requested ∩ available ∪ {metado}
    let avail = registry_permissions();
    let requested = vec![
        "storage.read".to_string(),
        "log.info".to_string(),
        "no.such.perm".to_string(),
    ];
    let exported = exported_namespaces(&requested, &avail);
    assert!(exported.contains(&"storage".to_string()));
    assert!(exported.contains(&"log".to_string()));
    assert!(exported.contains(&"metado".to_string()));
    assert!(!exported.contains(&"no".to_string()), "unrequestable perm must not export a namespace");
}

#[test]
fn contract_unexported_namespace_breaks_its_import() {
    // manifest 未请求 http 且无 --grant → import { http } 链接失败
    let bytes = bundle_bytes(
        "name = \"p\"\nversion = \"1.0.0\"\npermission = [\"log.info\"]\n[entries.boot]\nexport = \"boot\"\n",
        "import { http } from '@metado/runtime';\nexport default { boot() { return typeof http; } };\n",
    );
    assert!(run_mdl(&bytes, &[]).is_err(), "ungranted http import must fail to load");
}

#[test]
fn contract_extra_grant_still_respects_available_bound() {
    // --grant 只能把 manifest.permission ∪ grant 并入 requested，但仍受 available 约束：
    // 不存在的权限名不产生命名空间。
    let bytes = bundle_bytes(
        "name = \"q\"\nversion = \"1.0.0\"\npermission = [\"storage.read\"]\n[entries.boot]\nexport = \"boot\"\n",
        "import { storage } from '@metado/runtime';\nexport default { boot() { return typeof storage; } };\n",
    );
    let out = run_mdl(&bytes, &["bogus.perm".to_string()]).unwrap();
    // storage 照常导出（命名空间对象带方法）；bogus.perm 不影响
    assert_eq!(
        out.result,
        metado_engine::Value::String("object".to_string())
    );
}

#[test]
fn contract_glob_requested_namespace_still_exported() {
    // C5 回归：请求 http.get.api.example（可用 http.get.api.*）→ http 命名空间照常导出
    let bytes = bundle_bytes(
        "name = \"g\"\nversion = \"1.0.0\"\npermission = [\"http.get.api.example\"]\n[entries.boot]\nexport = \"boot\"\n",
        "import { http } from '@metado/runtime';\nexport default { boot() { return typeof http.get; } };\n",
    );
    let out = run_mdl(&bytes, &[]).unwrap();
    assert_eq!(
        out.result,
        metado_engine::Value::String("function".to_string())
    );
}

#[test]
fn contract_env_ungranted_requests_listed() {
    let bytes = bundle_bytes(
        "name = \"e\"\nversion = \"1.0.0\"\npermission = [\"log.info\", \"ghost.read\"]\n[entries.boot]\nexport = \"boot\"\n",
        "export default { boot() { return 1; } };\n",
    );
    let report = env_mdl(&bytes).unwrap();
    assert!(report.ungranted_requests.contains(&"ghost.read".to_string()));
    assert!(!report.exported_namespaces.contains(&"ghost".to_string()));
    assert!(report.exported_namespaces.contains(&"log".to_string()));
}

#[test]
fn contract_boot_and_test_agree_on_granted_export() {
    // run 与 test 的导出计算同源（metadata + grant） → 对同一授权结果一致
    let main = "import { storage } from '@metado/runtime';\nexport default { boot() { return typeof storage; }, test() { return typeof storage.read === 'function'; } };\n";
    let bytes = bundle_bytes(
        "name = \"t\"\nversion = \"1.0.0\"\npermission = [\"storage.read\"]\n[entries.boot]\nexport = \"boot\"\n[entries.test]\nexport = \"test\"\n",
        main,
    );
    let run = run_mdl(&bytes, &[]).unwrap();
    assert_eq!(run.result, metado_engine::Value::String("object".to_string()));
    let outcomes = test_mdl(&bytes, &[]).unwrap();
    assert!(outcomes.iter().any(|o| o.passed), "test must pass when export granted");
}