//! Metado crypto 能力（Task 3.6）：randomBytes / sha256 / hmac（WebCrypto 形状）。
//! HMAC 使用 HMAC-SHA-256；所有返回为原始字节（由上层按需编码）。

use metado_engine::capability::{CapabilityMeta, CapabilitySet};

pub struct MetaCrypto;

impl CapabilitySet for MetaCrypto {
    fn meta(&self) -> CapabilityMeta {
        CapabilityMeta {
            name: "crypto".into(),
            permissions: vec![
                "crypto.randomBytes".into(),
                "crypto.sha256".into(),
                "crypto.hmac".into(),
            ],
            exports: vec!["randomBytes".into(), "sha256".into(), "hmac".into()],
        }
    }
}

pub fn random_bytes(n: usize) -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::rng();
    let mut out = vec![0u8; n];
    rng.fill_bytes(&mut out);
    out
}

pub fn sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    Sha256::digest(data).into()
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, KeyInit, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta() {
        let meta = MetaCrypto.meta();
        assert_eq!(meta.name, "crypto");
        assert_eq!(meta.permissions, vec!["crypto.randomBytes", "crypto.sha256", "crypto.hmac"]);
        assert_eq!(meta.exports, vec!["randomBytes", "sha256", "hmac"]);
    }
}