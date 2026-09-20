//! Task 4.7: mdl trace —— 执行轨迹观测（§12.6）

use metado_cli::{build_plugin, trace_mdl};
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

fn build_with(manifest: &str, main_js: &str) -> (TempDir, Vec<u8>) {
    let dir = TempDir::new("metado-cli-trace").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("mdl.toml"), manifest).unwrap();
    std::fs::write(root.join("src/main.js"), main_js).unwrap();
    let bytes = build_plugin(&root, &KeyPair::generate()).unwrap();
    (dir, bytes)
}

#[test]
fn test_trace_records_entry_lifecycle() {
    let (_, bytes) = build_with(
        "name = \"t\"\nversion = \"1.0.0\"\npermission = [\"storage.read\"]\n[entries.boot]\nexport = \"boot\"\n",
        "export default { boot() { return 1; } };\n",
    );
    let events = trace_mdl(&bytes, &[], None).unwrap();
    let kinds: Vec<&str> = events.iter().map(|e| e.kind()).collect();
    assert!(kinds.contains(&"EntryStart"), "got {:?}", kinds);
    assert!(kinds.contains(&"EntryEnd"), "got {:?}", kinds);
}

#[test]
fn test_trace_records_capability_call_for_exported_runtime() {
    let (_, bytes) = build_with(
        "name = \"t\"\nversion = \"1.0.0\"\npermission = [\"storage.read\"]\n[entries.boot]\nexport = \"boot\"\n",
        "import { storage } from '@metado/runtime';\nexport default { boot() { return typeof storage; } };\n",
    );
    let events = trace_mdl(&bytes, &[], None).unwrap();
    let caps: Vec<&str> = events
        .iter()
        .filter(|e| e.kind() == "CapabilityCall")
        .map(|e| e.capability())
        .collect();
    assert!(caps.contains(&"storage"), "got {:?}", caps);
    assert!(caps.contains(&"metado"), "got {:?}", caps);
}

#[test]
fn test_trace_filter_by_kind() {
    let (_, bytes) = build_with(
        "name = \"t\"\nversion = \"1.0.0\"\npermission = [\"log.info\"]\n[entries.test]\nexport = \"test\"\n",
        "export default { test() { return true; } };\n",
    );
    let events = trace_mdl(&bytes, &[], Some("EntryStart")).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind(), "EntryStart");
}

#[test]
fn test_trace_no_export_no_entries_blocked() {
    let (_, bytes) = build_with("name = \"t\"\nversion = \"1.0.0\"\n", "export default {};\n");
    // 有 boot 回退；无任何入口仍可 trace（无 entry 事件被记录但 harness 不报错）
    let events = trace_mdl(&bytes, &[], None).unwrap();
    assert!(!events.iter().any(|e| e.kind() == "EntryStart"));
}