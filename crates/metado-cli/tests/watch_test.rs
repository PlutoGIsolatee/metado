//! Task 4.4: mdl watch 可测核心（防抖调度 + 增量重建组合）

use std::time::Duration;

use metado_cli::{build_and_run, WatchScheduler};
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

#[test]
fn test_debounce_defers_first_flood_of_changes() {
    let mut sch = WatchScheduler::new(Duration::from_millis(50));
    // 50ms 窗口内连续落盘事件 → 不应立即触发重建
    for _ in 0..5 {
        assert_eq!(sch.on_change(), false, "flood within debounce window must not fire");
    }
    std::thread::sleep(Duration::from_millis(80));
    assert_eq!(sch.on_change(), true, "after debounce elapses, pending change fires");
}

#[test]
fn test_record_reload_resets() {
    let mut sch = WatchScheduler::new(Duration::from_millis(30));
    // 第一次变更进入防抖窗口，窗口内不触发
    assert_eq!(sch.on_change(), false);
    std::thread::sleep(Duration::from_millis(50));
    // 挂起变更超时 → 触发
    assert_eq!(sch.on_change(), true);
    sch.record_reload();
    // 未再变化 → 不触发
    assert_eq!(sch.on_change(), false);
}

#[test]
fn test_build_and_run_returns_info() {
    let dir = TempDir::new("metado-cli-watch").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("mdl.toml"), "name = \"watchdemo\"\nversion = \"1.0.0\"\n").unwrap();
    std::fs::write(root.join("src/main.js"), "export default {};\n").unwrap();

    let kp = KeyPair::generate();
    let info = build_and_run(&root, &kp).unwrap();
    assert_eq!(info.plugin_name, "watchdemo");
}