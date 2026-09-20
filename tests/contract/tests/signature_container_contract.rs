//! Task 6.4 契约：签名校验 + 容器格式。

use metado_contract::common::{bundle_bytes, plugin_dir};
use metado_engine::{signer_id, Container, KeyPair, SignedBundle};
use metado_cli::{build_plugin, sign_mdl, verify_mdl};

#[test]
fn contract_sign_verify_roundtrip_and_signer_id_stable() {
    let bytes = bundle_bytes(
        "name = \"s\"\nversion = \"1.0.0\"\n",
        "export default {};\n",
    );
    let bundle = SignedBundle::from_bytes(&bytes).unwrap();
    bundle.verify().unwrap();
    // signer_id 是 pubkey 的稳定导出（16 hex 前缀 + 完整串可复算）
    assert_eq!(signer_id(&bundle.signer_pubkey()), verify_mdl(&bytes).unwrap().signer);
}

#[test]
fn contract_any_bit_flip_breaks_verify() {
    let bytes = bundle_bytes("name = \"x\"\nversion = \"1.0.0\"\n", "export default {};\n");
    for bit in [0usize, 4, 17, bytes.len() - 1] {
        let mut bad = bytes.clone();
        bad[bit] ^= 0x01;
        // 位翻转要么破坏魔数/头（from_bytes 拒绝），要么破坏负载签名（verify 拒绝）
        let rejected = match SignedBundle::from_bytes(&bad) {
            Ok(b) => b.verify().is_err(),
            Err(_) => true,
        };
        assert!(rejected, "bit flip at {} must be rejected", bit);
    }
}

#[test]
fn contract_resign_changes_signer_keeps_payload() {
    let kp1 = KeyPair::generate();
    let dir = plugin_dir(
        "rs",
        "name = \"r\"\nversion = \"1.0.0\"\n",
        "export default {};\n",
    );
    let bytes = build_plugin(&dir.path().join("plugin"), &kp1).unwrap();
    let b1 = SignedBundle::from_bytes(&bytes).unwrap();
    let payload1 = b1.payload().to_vec();
    let id1 = signer_id(&b1.signer_pubkey());

    let kp2 = KeyPair::generate();
    let resigned = sign_mdl(&bytes, &kp2).unwrap();
    let b2 = SignedBundle::from_bytes(&resigned).unwrap();
    b2.verify().unwrap();
    assert_eq!(b2.payload(), &payload1[..], "re-sign must not alter payload");
    assert_ne!(signer_id(&b2.signer_pubkey()), id1);
}

#[test]
fn contract_verify_rejects_wrong_key() {
    let kp = KeyPair::generate();
    let dir = plugin_dir("wk", "name = \"w\"\nversion = \"1.0.0\"\n", "export default {};\n");
    let bytes = build_plugin(&dir.path().join("plugin"), &kp).unwrap();
    let b = SignedBundle::from_bytes(&bytes).unwrap();
    let wrong = KeyPair::generate();
    assert!(b.verify_with_key(&wrong.public_key_bytes()).is_err());
    assert!(b.verify_with_key(&kp.public_key_bytes()).is_ok());
}

#[test]
fn contract_container_roundtrip_preserves_files() {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cursor);
    let opts = zip::write::SimpleFileOptions::default();
    let entries = [
        ("mdl.toml", b"name = \"c\"\n".to_vec()),
        ("src/main.js", b"export default {};\n".to_vec()),
        ("assets/data.txt", b"payload".to_vec()),
    ];
    for (name, data) in &entries {
        zw.start_file(name, opts).unwrap();
        std::io::Write::write_all(&mut zw, data).unwrap();
    }
    let cursor = zw.finish().unwrap();
    let container = Container::from_bytes(&cursor.into_inner()).unwrap();
    for (name, _) in &entries {
        assert!(container.read_file(name).is_ok(), "{} readable", name);
    }
    assert_eq!(container.read_file("assets/data.txt").unwrap(), b"payload".to_vec());
    assert!(container.read_file("no/such").is_err());
}

#[test]
fn contract_container_rejects_dotdot_entries() {
    // 恶意路径 `../` 必须在容器解析阶段被拒
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zw = zip::ZipWriter::new(cursor);
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file("mdl.toml", opts).unwrap();
    std::io::Write::write_all(&mut zw, b"name = \"d\"\n").unwrap();
    zw.start_file("../../escape.txt", opts).unwrap();
    std::io::Write::write_all(&mut zw, b"nope").unwrap();
    let cursor = zw.finish().unwrap();
    assert!(Container::from_bytes(&cursor.into_inner()).is_err(), "dotdot entry must be rejected");
}

#[test]
fn contract_build_requires_mdl_toml() {
    let dir = plugin_dir("mm", "name = \"m\"\nversion = \"1.0.0\"\n", "export default {};\n");
    let root = dir.path().join("plugin");
    std::fs::remove_file(root.join("mdl.toml")).unwrap();
    assert!(build_plugin(&root, &KeyPair::generate()).is_err());
}