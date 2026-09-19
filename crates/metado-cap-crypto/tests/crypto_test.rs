//! Task 3.6: metado-cap-crypto tests

use metado_cap_crypto::{hmac_sha256, random_bytes, sha256, MetaCrypto};
use metado_engine::capability::CapabilitySet;

#[test]
fn test_meta() {
    let meta = MetaCrypto.meta();
    assert_eq!(meta.name, "crypto");
    for perm in [
        "crypto.randomBytes",
        "crypto.sha256",
        "crypto.hmac",
    ] {
        assert!(meta.permissions.contains(&perm.to_string()), "missing {}", perm);
    }
    for exp in ["randomBytes", "sha256", "hmac"] {
        assert!(meta.exports.contains(&exp.to_string()), "missing {}", exp);
    }
}

#[test]
fn test_sha256_known_vector() {
    // NIST FIPS 180-2: SHA-256("abc")
    let digest = sha256(b"abc");
    assert_eq!(
        hex::encode(digest),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn test_sha256_empty() {
    assert_eq!(
        hex::encode(sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn test_hmac_sha256_known_vector() {
    // RFC 4231 test case 1
    let tag = hmac_sha256(b"key", b"The quick brown fox jumps over the lazy dog");
    assert_eq!(
        hex::encode(tag),
        "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
    );
}

#[test]
fn test_random_bytes() {
    let a = random_bytes(32);
    let b = random_bytes(32);
    assert_eq!(a.len(), 32);
    assert_eq!(b.len(), 32);
    assert_ne!(a, b);
}