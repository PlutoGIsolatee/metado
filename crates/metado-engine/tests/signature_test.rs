//! Task 1.4: Ed25519 signature tests (§6)

use metado_engine::signature::{signer_id, KeyPair, SignedBundle};

#[test]
fn test_sign_and_verify() {
    let kp = KeyPair::generate();
    let payload = b"hello world";
    let signed = SignedBundle::sign(&kp, payload);
    assert!(signed.verify().is_ok());
    assert_eq!(signed.payload(), payload);
}

#[test]
fn test_signer_id() {
    let kp = KeyPair::generate();
    let id = signer_id(&kp.public_key_bytes());
    assert_eq!(id.len(), 32); // 公钥指纹: sha256 截段 16 bytes = 32 hex chars
}

#[test]
fn test_tampered_payload_fails() {
    let kp = KeyPair::generate();
    let mut signed = SignedBundle::sign(&kp, b"original");
    // Tamper with payload
    signed.payload_mut()[0] = 0xff;
    assert!(signed.verify().is_err());
}

#[test]
fn test_wrong_key_fails() {
    let kp1 = KeyPair::generate();
    let kp2 = KeyPair::generate();
    let signed = SignedBundle::sign(&kp1, b"hello");
    assert!(signed.verify_with_key(&kp2.public_key_bytes()).is_err());
}

#[test]
fn test_to_bytes_roundtrip() {
    let kp = KeyPair::generate();
    let signed = SignedBundle::sign(&kp, b"persist me");
    let bytes = signed.to_bytes();
    let restored = SignedBundle::from_bytes(&bytes).unwrap();
    assert_eq!(restored.payload(), b"persist me");
    assert!(restored.verify().is_ok());
}

#[test]
fn test_from_bytes_rejects_bad_magic() {
    let kp = KeyPair::generate();
    let mut bytes = SignedBundle::sign(&kp, b"x").to_bytes();
    bytes[0] = b'X';
    assert!(SignedBundle::from_bytes(&bytes).is_err());
}