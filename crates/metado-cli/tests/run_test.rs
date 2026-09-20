//! Task 4.3: mdl run 主机接线测试
//! v1 执行语义：真实 ESM（PluginRuntime），与 mdl test/trace/daemon 一致。

use metado_cli::{build_plugin, run_mdl};
use metado_engine::{KeyPair, Value};

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

fn build_boot_plugin() -> (TempDir, Vec<u8>) {
    let dir = TempDir::new("metado-cli-run").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("mdl.toml"),
        "name = \"demo\"\nversion = \"1.0.0\"\n\
         permission = [\"log.info\", \"storage.<signer>.read\"]\n\
         [entries.boot]\nexport = \"boot\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.js"),
        "export default { boot() { return \"hello\"; } };\n",
    )
    .unwrap();
    let bytes = build_plugin(&root, &KeyPair::generate()).unwrap();
    (dir, bytes)
}

#[test]
fn test_run_activates_and_invokes_boot() {
    let (_g, bytes) = build_boot_plugin();
    let outcome = run_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcome.plugin_id, "demo");
    assert_eq!(outcome.entry, "boot");
    assert_eq!(outcome.invoked, true);
    assert_eq!(outcome.result, Value::String("hello".to_string()));
}

#[test]
fn test_run_real_esm_imports_runtime() {
    let dir = TempDir::new("metado-cli-run").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("mdl.toml"),
        "name = \"esm\"\nversion = \"1.0.0\"\npermission = [\"storage.read\", \"log.info\"]\n\
         [entries.boot]\nexport = \"boot\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/main.js"),
        "import { storage, log } from \"@metado/runtime\";\n\
         export default { boot() { return typeof storage + \"/\" + typeof log; } };\n",
    )
    .unwrap();
    let bytes = build_plugin(&root, &KeyPair::generate()).unwrap();

    // 未请求 http → 不导出；请求的 storage/log → 导出（stub 类型 function）
    let outcome = run_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcome.result, Value::String("function/function".to_string()));

    // 显式 --grant 只能追加已请求；未请求的 http 不因 grant 单列出现
    // （导出集合以 requested=次为准，extra_grant 并入 requested）
    let outcome = run_mdl(&bytes, &["http.fetch".to_string()]).unwrap();
    assert_eq!(outcome.result, Value::String("function/function".to_string()));
}

#[test]
fn test_run_tampered_fails() {
    let (_g, bytes) = build_boot_plugin();
    let mut bad = bytes;
    let last = bad.len() - 1;
    bad[last] ^= 0xFF;
    assert!(run_mdl(&bad, &[]).is_err());
}

#[test]
fn test_run_extra_grant_accepted() {
    let (_g, bytes) = build_boot_plugin();
    let extra = vec!["http.get.api.external".to_string()];
    assert!(run_mdl(&bytes, &extra).is_ok());
}

#[test]
fn test_run_plugin_without_boot_is_noop() {
    let dir = TempDir::new("metado-cli-run").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("mdl.toml"),
        "name = \"plain\"\nversion = \"1.0.0\"\n",
    )
    .unwrap();
    std::fs::write(root.join("src/main.js"), "export default {};\n").unwrap();
    let bytes = build_plugin(&root, &KeyPair::generate()).unwrap();

    let outcome = run_mdl(&bytes, &[]).unwrap();
    assert_eq!(outcome.plugin_id, "plain");
    assert_eq!(outcome.invoked, false);
}