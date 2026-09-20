//! Metado CLI 核心逻辑（Phase 4）：key 管理 + build/verify/sign/run。
//! - `mdl build <plugin-dir>`：目录 → ZIP → 签名 → .mdl（SignedBundle 字节即磁盘格式）
//! - `mdl verify <file>`：校验签名并回读 manifest
//! - `mdl sign <file> [--key FILE]`：用指定密钥重置签名者
//! - `mdl run <file.mdl>`：进程内引擎，grant 后 invoke boot 入口
//! 主机密钥文件 = 64 hex 字符（32-byte ed25519 种子），默认 ~/.metado/keys/default.key

mod run;
mod test;
mod watch;

pub use run::{run_mdl, RunOutcome};
pub use test::{test_mdl, TestOutcome};
pub use watch::{build_and_run, WatchScheduler};
#[cfg(feature = "watch")]
pub use watch::run_watch;

use std::io::Write;
use std::path::{Path, PathBuf};

use rand::Rng;

use metado_engine::{signer_id, Container, KeyPair, Manifest, SignedBundle};

const MANIFEST_FILE: &str = "mdl.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStoreError {
    Missing,
    InvalidHex,
    InvalidLength,
}

pub struct KeyStore;

impl KeyStore {
    /// 生成新密钥并写 hex 文件到 `path`（父目录自动创建）。
    pub fn generate(path: &Path) -> Result<KeyPair, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir keys: {}", e))?;
        }
        let mut seed = [0u8; 32];
        rand::rng().fill_bytes(&mut seed);
        let kp = KeyPair::from_bytes(&seed);
        let hex_str = hex::encode(&seed);
        std::fs::write(path, hex_str).map_err(|e| format!("write key: {}", e))?;
        Ok(kp)
    }

    pub fn load(path: &Path) -> Result<KeyPair, KeyStoreError> {
        let raw = std::fs::read(path).map_err(|_| KeyStoreError::Missing)?;
        let s = String::from_utf8(raw).map_err(|_| KeyStoreError::InvalidHex)?;
        let bytes = hex::decode(s.trim()).map_err(|_| KeyStoreError::InvalidHex)?;
        if bytes.len() != 32 {
            return Err(KeyStoreError::InvalidLength);
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        Ok(KeyPair::from_bytes(&seed))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    pub plugin_name: String,
    pub signer: String,
}

/// 把插件目录构建为签名后的 .mdl 字节（Task 4.1）。
pub fn build_plugin(dir: &Path, kp: &KeyPair) -> Result<Vec<u8>, String> {
    let zip_bytes = build_zip(dir)?;
    let bundle = SignedBundle::sign(kp, &zip_bytes);
    Ok(bundle.to_bytes())
}

/// 目录 → 元数据适配 zip 字节（含 mdl.toml，排除隐藏文件/目录穿越）。
fn build_zip(dir: &Path) -> Result<Vec<u8>, String> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    collect_files(dir, dir, &mut files)?;
    if !files.iter().any(|(p, _)| p == MANIFEST_FILE) {
        return Err(format!("plugin dir missing {}: {}", MANIFEST_FILE, dir.display()));
    }

    let cursor = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default();
    for (name, data) in &files {
        zw.start_file(name, options)
            .map_err(|e| format!("zip entry {}: {}", name, e))?;
        zw.write_all(data).map_err(|e| format!("zip write {}: {}", name, e))?;
    }
    let cursor = zw.finish().map_err(|e| format!("zip finish: {}", e))?;
    Ok(cursor.into_inner())
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("read dir {}: {}", dir.display(), e))? {
        let entry = entry.map_err(|e| format!("readdir: {}", e))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        if name.starts_with('.') || name == "mdl.key" {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .map_err(|_| "path escape while walking plugin dir".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if rel.contains("..") {
                return Err(format!("unsafe plugin path: {}", rel));
            }
            let data = std::fs::read(&path).map_err(|e| format!("read {}: {}", rel, e))?;
            out.push((rel, data));
        }
    }
    Ok(())
}

/// 校验 .mdl 签名并回读信息（Task 4.2）。
pub fn verify_mdl(bytes: &[u8]) -> Result<BuildInfo, String> {
    let bundle = SignedBundle::from_bytes(bytes)?;
    bundle.verify()?;
    let container = Container::from_bytes(bundle.payload())?;
    let manifest_raw = container.read_file(MANIFEST_FILE)?;
    let manifest_raw = String::from_utf8(manifest_raw).map_err(|e| format!("manifest utf8: {}", e))?;
    let manifest = Manifest::from_toml(&manifest_raw).map_err(|e| format!("manifest: {}", e))?;
    Ok(BuildInfo {
        plugin_name: manifest.name,
        signer: signer_id(&bundle.signer_pubkey()),
    })
}

/// 用新密钥重新签名既有 .mdl（payload 不变，仅换签名者）。
pub fn sign_mdl(bytes: &[u8], kp: &KeyPair) -> Result<Vec<u8>, String> {
    let bundle = SignedBundle::from_bytes(bytes)?;
    let rebundled = SignedBundle::sign(kp, bundle.payload());
    Ok(rebundled.to_bytes())
}

/// 默认主机密钥路径。
pub fn default_key_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".metado/keys/default.key")
}