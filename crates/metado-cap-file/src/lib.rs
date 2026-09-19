//! Metado file 能力（Task 3.3）：**只读**文件访问，根目录 confinement。
//! 所有读取被限制在许可根目录内；任何 `..`/绝对路径逃逸一律拒绝。
//! 写/删不提供——需要写盘的宿主应使用 storage 能力。

use std::path::PathBuf;

use metado_engine::capability::{CapabilityMeta, CapabilitySet};

pub struct MetaFile;

impl CapabilitySet for MetaFile {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "file".into(),
            permissions: vec!["file.read".into(), "file.readText".into()],
            exports: vec!["read".into(), "readText".into()],
        }
    }
}

pub struct FileAccess {
    root: PathBuf,
}

impl FileAccess {
    pub fn new(root: &str) -> Result<Self, String> {
        let root = PathBuf::from(root);
        if !root.is_dir() {
            return Err(format!("file root is not a directory: {}", root.display()));
        }
        Ok(Self { root })
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        let full = self.resolve(path)?;
        std::fs::read(&full).map_err(|e| format!("file read {}: {}", path, e))
    }

    pub fn read_text(&self, path: &str) -> Result<String, String> {
        let bytes = self.read(path)?;
        String::from_utf8(bytes).map_err(|e| format!("file {} not UTF-8: {}", path, e))
    }

    fn resolve(&self, rel: &str) -> Result<PathBuf, String> {
        if rel.contains("..") || rel.starts_with('/') {
            return Err(format!("path escape blocked: {}", rel));
        }
        let full = self.root.join(rel);
        Ok(full)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_basic() {
        let meta = MetaFile.meta();
        assert_eq!(meta.name, "file");
    }
}