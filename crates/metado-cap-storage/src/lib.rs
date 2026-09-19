//! Metado storage 能力（Task 3.2）：明文文件系统后端。
//! - signer 命名空间：`storage.<signer>.read/write`（同发布者共享 keyspace）
//! - 私有 keyspace：`storage:<plugin_id>:<key>`；共享 keyspace：`storage:<signer_id>:<key>`
//! - 路径穿越一律拒绝（key 不得含 `..` 或 `/`）

use std::path::PathBuf;

use metado_engine::capability::{CapabilityMeta, CapabilitySet};

pub struct MetaStorage;

impl CapabilitySet for MetaStorage {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "storage".into(),
            permissions: vec![
                "storage.read".into(),
                "storage.write".into(),
                "storage.<signer>.read".into(),
                "storage.<signer>.write".into(),
            ],
            exports: vec!["read".into(), "write".into()],
        }
    }
}

/// 私有 keyspace：`storage:<plugin_id>:<key>`
pub fn private_key(plugin_id: &str, key: &str) -> String {
    format!("storage:{}:{}", plugin_id, key)
}

/// 共享 keyspace：`storage:<signer_id>:<key>`（同发布者同 signer 互通）
pub fn shared_key(signer_id: &str, key: &str) -> String {
    format!("storage:{}:{}", signer_id, key)
}

pub struct Storage {
    base_dir: PathBuf,
}

impl Storage {
    pub fn new(base_dir: &str) -> Result<Self, String> {
        let dir = PathBuf::from(base_dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("init storage: {}", e))?;
        Ok(Self { base_dir: dir })
    }

    pub fn read(&self, key: &str) -> Result<Vec<u8>, String> {
        let path = self.key_path(key)?;
        std::fs::read(&path).map_err(|e| format!("storage read {}: {}", key, e))
    }

    pub fn write(&self, key: &str, data: &[u8]) -> Result<(), String> {
        let path = self.key_path(key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("storage mkdir: {}", e))?;
        }
        std::fs::write(&path, data).map_err(|e| format!("storage write {}: {}", key, e))
    }

    fn key_path(&self, key: &str) -> Result<PathBuf, String> {
        if key.contains("..") || key.contains('/') || key.contains('\\') {
            return Err(format!("invalid storage key: {}", key));
        }
        if key.is_empty() {
            return Err("empty storage key".into());
        }
        Ok(self.base_dir.join(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_basic() {
        let meta = MetaStorage.meta();
        assert_eq!(meta.name, "storage");
    }
}