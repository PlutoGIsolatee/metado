//! Task 2.2: Node-style module resolution tests

use std::fs;
use std::path::PathBuf;

use metado_executor::ModuleLoader;

const SRC_MAIN: &str = "export function main() {}";
const HELPER: &str = "export function helper() {}";
const LODASH: &str = "module.exports = {};";

fn setup() -> (tempfile_guard::TempDir, String) {
    // 用真实临时目录建文件树，避免计划中虚构 /plugin 路径在真机 FS 上不存在
    let dir = tempfile_guard::TempDir::new("metado-module-test").unwrap();
    let root = dir.path().to_str().unwrap().to_string();

    fs::create_dir_all(format!("{}/plugin/src", root)).unwrap();
    fs::create_dir_all(format!("{}/plugin/node_modules/lodash", root)).unwrap();
    fs::write(format!("{}/plugin/src/main.js", root), SRC_MAIN).unwrap();
    fs::write(format!("{}/plugin/src/helper.js", root), HELPER).unwrap();
    fs::write(format!("{}/plugin/node_modules/lodash/index.js", root), LODASH).unwrap();
    (dir, root)
}

mod tempfile_guard {
    use std::path::PathBuf;

    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> std::io::Result<Self> {
            // 并行测试共享同进程 PID，目录名需逐次唯一，否则先完成的测试 Drop 会
            // 删除后执行测试正在使用的共享目录
            static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut base = std::env::temp_dir();
            base.push(format!("{}-{}-{}", tag, std::process::id(), n));
            std::fs::create_dir_all(&base)?;
            Ok(Self(base))
        }

        pub fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[test]
fn test_resolve_relative() {
    let (_g, root) = setup();
    let loader = ModuleLoader::new(&format!("{}/plugin", root));
    let from = format!("{}/plugin/src/on_message.js", root);
    let resolved = loader.resolve("./helper.js", &from).unwrap();
    assert_eq!(resolved, PathBuf::from(format!("{}/plugin/src/helper.js", root)));
}

#[test]
fn test_resolve_node_modules() {
    let (_g, root) = setup();
    let loader = ModuleLoader::new(&format!("{}/plugin", root));
    let from = format!("{}/plugin/src/main.js", root);
    let resolved = loader.resolve("lodash", &from).unwrap();
    assert!(resolved.to_string_lossy().contains("node_modules/lodash"));
}

#[test]
fn test_resolve_extension() {
    let (_g, root) = setup();
    let loader = ModuleLoader::new(&format!("{}/plugin", root));
    let from = format!("{}/plugin/src/main.js", root);
    let resolved = loader.resolve("./helper", &from).unwrap();
    assert_eq!(
        resolved,
        PathBuf::from(format!("{}/plugin/src/helper.js", root))
    );
}

#[test]
fn test_resolve_absolute() {
    let (_g, root) = setup();
    let loader = ModuleLoader::new(&format!("{}/plugin", root));
    let from = format!("{}/plugin/src/main.js", root);
    let abs = format!("{}/usr/lib/util.js", root);
    let resolved = loader.resolve(&abs, &from).unwrap();
    assert_eq!(resolved, PathBuf::from(&abs));
}

#[test]
fn test_resolve_missing() {
    let (_g, root) = setup();
    let loader = ModuleLoader::new(&format!("{}/plugin", root));
    let from = format!("{}/plugin/src/main.js", root);
    assert!(loader.resolve("./nope", &from).is_err());
}