//! 契约测试共享夹具：构造 .mdl 字节与临时目录。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use metado_engine::{KeyPair, SignedBundle};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct TempDir(PathBuf);
impl TempDir {
    pub fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut base = std::env::temp_dir();
        base.push(format!("metado-contract-{}-{}-{}", tag, std::process::id(), n));
        std::fs::create_dir_all(&base).unwrap();
        Self(base)
    }
    pub fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn plugin_dir(tag: &str, manifest: &str, main_js: &str) -> TempDir {
    let dir = TempDir::new(tag);
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("mdl.toml"), manifest).unwrap();
    std::fs::write(root.join("src/main.js"), main_js).unwrap();
    dir
}

pub fn bundle_bytes(manifest: &str, main_js: &str) -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cursor);
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file("mdl.toml", opts).unwrap();
    zw.write_all(manifest.as_bytes()).unwrap();
    zw.start_file("src/main.js", opts).unwrap();
    zw.write_all(main_js.as_bytes()).unwrap();
    let cursor = zw.finish().unwrap();
    SignedBundle::sign(&KeyPair::generate(), &cursor.into_inner()).to_bytes()
}

pub fn hex_str(bytes: &[u8]) -> String {
    hex::encode(bytes)
}