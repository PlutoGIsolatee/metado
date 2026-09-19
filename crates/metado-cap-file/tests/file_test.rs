//! Task 3.3: metado-cap-file tests

use metado_cap_file::{FileAccess, MetaFile};
use metado_engine::capability::CapabilitySet;

fn temp_file(tag: &str) -> (TempDir, String) {
    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut base = std::env::temp_dir();
    base.push(format!("{}-{}-{}", tag, std::process::id(), n));
    std::fs::create_dir_all(&base).unwrap();
    let s = base.to_str().unwrap().to_string();
    (TempDir(base), s)
}

struct TempDir(std::path::PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_meta() {
    let meta = MetaFile.meta();
    assert_eq!(meta.name, "file");
    for perm in ["file.read", "file.readText"] {
        assert!(meta.permissions.contains(&perm.to_string()), "missing {}", perm);
    }
}

#[test]
fn test_read_binary() {
    let (_g, base) = temp_file("metado-file");
    std::fs::write(format!("{}/a.bin", base), [0u8, 1, 255]).unwrap();

    let fa = FileAccess::new(&base).unwrap();
    assert_eq!(fa.read("a.bin").unwrap(), vec![0u8, 1, 255]);
}

#[test]
fn test_read_text() {
    let (_g, base) = temp_file("metado-file");
    std::fs::write(format!("{}/a.txt", base), "你好 world").unwrap();

    let fa = FileAccess::new(&base).unwrap();
    assert_eq!(fa.read_text("a.txt").unwrap(), "你好 world");
}

#[test]
fn test_path_escaping_blocked() {
    let (_g, base) = temp_file("metado-file");
    std::fs::write(format!("{}/sec.txt", base), "top secret").unwrap();

    let fa = FileAccess::new(&base).unwrap();
    // 试图读取 base 之外
    assert!(fa.read("../__metado_escape_check__").is_err());
    // base 内合法
    assert_eq!(fa.read_text("sec.txt").unwrap(), "top secret");
}

#[test]
fn test_read_missing() {
    let (_g, base) = temp_file("metado-file");
    let fa = FileAccess::new(&base).unwrap();
    assert!(fa.read_text("missing.txt").is_err());
}