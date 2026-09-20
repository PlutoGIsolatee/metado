//! Task 4.5: mdl test —— 真实 ESM 执行下的插件契约测试

use metado_cli::{build_plugin, test_mdl};
use metado_engine::KeyPair;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new(tag: &str) -> std::io::Result<Self> {
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut base = std::env::temp_dir();
        base.push(format!("{}-{}-{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&base)?;
        Ok(Self(base))
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn build_with_main(manifest: &str, main_js: &str) -> (TempDir, Vec<u8>) {
    let dir = TempDir::new("metado-cli-test").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("mdl.toml"), manifest).unwrap();
    std::fs::write(root.join("src/main.js"), main_js).unwrap();
    let bytes = build_plugin(&root, &KeyPair::generate()).unwrap();
    (dir, bytes)
}

#[test]
fn test_pass_case() {
    let (_, bytes) = build_with_main(
        "name = \"t\"\nversion = \"1.0.0\"\n[entries.test]\nexport = \"test\"\n",
        "export default {\n  test() {\n    if (2 + 2 !== 4) throw new Error('bad math');\n    return true;\n  }\n};\n",
    );
    let outcomes = test_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcomes.len(), 1);
    let o = &outcomes[0];
    assert_eq!(o.name, "test");
    assert!(o.passed, "expected pass, got {:?}", o);
}

#[test]
fn test_fail_case() {
    let (_, bytes) = build_with_main(
        "name = \"t\"\nversion = \"1.0.0\"\n[entries.test]\nexport = \"test\"\n",
        "export default {\n  test() {\n    return 0;\n  }\n};\n",
    );
    let outcomes = test_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcomes[0].passed, false);
}

#[test]
fn test_throwing_case_reports_fail_not_harness_error() {
    let (_, bytes) = build_with_main(
        "name = \"t\"\nversion = \"1.0.0\"\n[entries.test]\nexport = \"test\"\n",
        "export default {\n  test() {\n    throw new Error('kaput');\n  }\n};\n",
    );
    // 用例抛错 → 该用例 FAIL，但 harness 本身不整体 Err（结果列表正常返回）
    let outcomes = test_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(!outcomes[0].passed);
    assert!(outcomes[0].detail.contains("kaput"));
}

#[test]
fn test_falls_back_to_boot_when_no_test_entry() {
    let (_, bytes) = build_with_main(
        "name = \"t\"\nversion = \"1.0.0\"\n[entries.boot]\nexport = \"boot\"\n",
        "export default {\n  boot() {\n    return 'primed';\n  }\n};\n",
    );
    let outcomes = test_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcomes[0].name, "boot");
    assert!(outcomes[0].passed);
}

#[test]
fn test_no_entry_reports_fail_not_blocking() {
    let (_, bytes) = build_with_main(
        "name = \"t\"\nversion = \"1.0.0\"\n",
        "export default {};\n",
    );
    let outcomes = test_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(!outcomes[0].passed);
    assert!(outcomes[0].detail.contains("declares no"));
}

#[test]
fn test_tampered_fails() {
    let (_, mut bytes) = build_with_main(
        "name = \"t\"\nversion = \"1.0.0\"\n",
        "export default {};\n",
    );
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    assert!(test_mdl(&bytes, &[]).is_err());
}