//! Task 4.6: mdl env —— 能力/权限静态诊断（§12.5）

use metado_cli::{build_plugin, env_mdl};
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

fn build_with(manifest: &str) -> (TempDir, Vec<u8>) {
    let dir = TempDir::new("metado-cli-env").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("mdl.toml"), manifest).unwrap();
    std::fs::write(
        root.join("src/main.js"),
        "export default { boot() { return 1; } };\n",
    )
    .unwrap();
    let bytes = build_plugin(&root, &KeyPair::generate()).unwrap();
    (dir, bytes)
}

const MANIFEST: &str = "name = \"envdemo\"\nversion = \"1.0.0\"\n\
    permission = [\"storage.read\", \"log.info\"]\n\
    permission-set = [\"standard\"]\n\
    [entries.boot]\nexport = \"boot\"\n\
    [entries.test]\nexport = \"test\"\n";

#[test]
fn test_report_basics() {
    let (_g, bytes) = build_with(MANIFEST);
    let report = env_mdl(&bytes).unwrap();
    assert_eq!(report.plugin_name, "envdemo");
    assert_eq!(report.requested, vec!["storage.read".to_string(), "log.info".to_string()]);
    assert_eq!(report.permission_sets, vec!["standard".to_string()]);
}

#[test]
fn test_exported_namespaces_only_requested_and_existing() {
    let (_g, bytes) = build_with(MANIFEST);
    let report = env_mdl(&bytes).unwrap();
    // storage/log 被请求且在内置能力集中 → 导出；http 未请求 → 不导出
    assert!(report.exported_namespaces.contains(&"storage".to_string()));
    assert!(report.exported_namespaces.contains(&"log".to_string()));
    assert!(report.exported_namespaces.contains(&"metado".to_string()));
    assert!(!report.exported_namespaces.contains(&"http".to_string()));
    assert!(!report.exported_namespaces.contains(&"crypto".to_string()));
}

#[test]
fn test_entry_table() {
    let (_g, bytes) = build_with(MANIFEST);
    let report = env_mdl(&bytes).unwrap();
    assert!(report.entries.contains(&("boot".to_string(), "boot".to_string())));
    assert!(report.entries.contains(&("test".to_string(), "test".to_string())));
}

#[test]
fn test_ungranted_requests_listed() {
    let (_g, bytes) = build_with(
        "name = \"envdemo\"\nversion = \"1.0.0\"\npermission = [\"http.get\", \"custom.alert\"]\n",
    );
    let report = env_mdl(&bytes).unwrap();
    assert!(report.ungranted_requests.contains(&"custom.alert".to_string()));
    // 被请求但注册表缺失 → 不导出
    assert!(!report.exported_namespaces.contains(&"custom".to_string()));
}

#[test]
fn test_tampered_fails() {
    let (_g, mut bytes) = build_with(MANIFEST);
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    assert!(env_mdl(&bytes).is_err());
}