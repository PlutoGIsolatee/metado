//! Task 3.2: metado-cap-storage tests

use metado_cap_storage::{private_key, shared_key, MetaStorage, Storage};
use metado_engine::capability::CapabilitySet;

fn temp_base(tag: &str) -> (TempDir, String) {
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
    let meta = MetaStorage.meta();
    assert_eq!(meta.name, "storage");
    for perm in [
        "storage.read",
        "storage.write",
        "storage.<signer>.read",
        "storage.<signer>.write",
    ] {
        assert!(meta.permissions.contains(&perm.to_string()), "missing {}", perm);
    }
}

#[test]
fn test_write_read_roundtrip() {
    let (_g, base) = temp_base("metado-storage");
    let storage = Storage::new(&base).unwrap();
    storage.write("profile", b"hello").unwrap();
    assert_eq!(storage.read("profile").unwrap(), b"hello");
}

#[test]
fn test_read_missing() {
    let (_g, base) = temp_base("metado-storage");
    let storage = Storage::new(&base).unwrap();
    assert!(storage.read("nope").is_err());
}

#[test]
fn test_path_traversal_blocked() {
    let (_g, base) = temp_base("metado-storage");
    let storage = Storage::new(&base).unwrap();
    assert!(storage.write("../escape", b"evil").is_err());
    assert!(storage.read("../../etc/passwd").is_err());
}

#[test]
fn test_keyspace_helpers() {
    assert_eq!(private_key("plugin1", "k"), "storage:plugin1:k");
    assert_eq!(shared_key("abc123", "k"), "storage:abc123:k");
}