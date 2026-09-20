//! Phase 4: metado-cli 核心逻辑测试（build / verify / sign，Task 4.1-4.2）

use metado_cli::{build_plugin, sign_mdl, verify_mdl, KeyStore, KeyStoreError};
use metado_engine::{Container, KeyPair, SignedBundle};

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

fn sample_plugin_dir() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new("metado-cli").unwrap();
    let root = dir.path().join("plugin");
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("mdl.toml"), "name = \"demo\"\nversion = \"1.0.0\"\n").unwrap();
    std::fs::write(
        root.join("src/main.js"),
        "export function boot() { return \"ok\"; }\n",
    )
    .unwrap();
    (dir, root)
}

#[test]
fn test_generate_and_load_key_roundtrip() {
    let dir = TempDir::new("metado-cli").unwrap();
    let path = dir.path().join("test.key");
    let kp1 = KeyStore::generate(&path).unwrap();
    let kp2 = KeyStore::load(&path).unwrap();
    assert_eq!(kp1.public_key_bytes(), kp2.public_key_bytes());
}

#[test]
fn test_load_missing_key_errors() {
    let dir = TempDir::new("metado-cli").unwrap();
    let path = dir.path().join("nope.key");
    match KeyStore::load(&path) {
        Err(KeyStoreError::Missing) => {}
        other => panic!("expected Missing, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn test_build_plugin_signed_and_verifiable() {
    let (_g, root) = sample_plugin_dir();
    let kp = KeyPair::generate();

    let bytes = build_plugin(&root, &kp).unwrap();

    let bundle = SignedBundle::from_bytes(&bytes).unwrap();
    assert!(bundle.verify().is_ok());
    assert_eq!(bundle.signer_pubkey(), kp.public_key_bytes());

    let container = Container::from_bytes(bundle.payload()).unwrap();
    assert!(container.file_exists("mdl.toml"));
    assert!(container.file_exists("src/main.js"));
}

#[test]
fn test_verify_mdl_ok() {
    let (_g, root) = sample_plugin_dir();
    let kp = KeyPair::generate();
    let bytes = build_plugin(&root, &kp).unwrap();

    let info = verify_mdl(&bytes).unwrap();
    assert_eq!(info.plugin_name, "demo");
    assert_eq!(info.signer, metado_engine::signer_id(&kp.public_key_bytes()));
}

#[test]
fn test_verify_mdl_tampered_fails() {
    let (_g, root) = sample_plugin_dir();
    let kp = KeyPair::generate();
    let mut bytes = build_plugin(&root, &kp).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    assert!(verify_mdl(&bytes).is_err());
}

#[test]
fn test_sign_mdl_replaces_signer() {
    let (_g, root) = sample_plugin_dir();
    let kp1 = KeyPair::generate();
    let mut bytes = build_plugin(&root, &kp1).unwrap();

    let kp2 = KeyPair::generate();
    bytes = sign_mdl(&bytes, &kp2).unwrap();

    let bundle = SignedBundle::from_bytes(&bytes).unwrap();
    assert!(bundle.verify().is_ok());
    assert_eq!(bundle.signer_pubkey(), kp2.public_key_bytes());
    assert_eq!(
        verify_mdl(&bytes).unwrap().signer,
        metado_engine::signer_id(&kp2.public_key_bytes())
    );
}