use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

pub const MDL_MAGIC: &[u8; 4] = b"MDL1";
pub const MDL_FORMAT_VERSION: u16 = 1;

pub struct KeyPair {
    signing_key: SigningKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        // rand >= 0.10: `thread_rng()` renamed to `rng()`
        let mut rng = rand::rng();
        Self {
            signing_key: SigningKey::generate(&mut rng),
        }
    }

    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(bytes),
        }
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }
}

pub fn signer_id(pubkey: &[u8; 32]) -> String {
    let hash = Sha256::digest(pubkey);
    hex::encode(&hash[..16]) // 截段 16 bytes = 32 hex chars
}

pub struct SignedBundle {
    header: Vec<u8>, // magic + version + pubkey + payload_len
    signature: Signature,
    payload: Vec<u8>,
}

impl SignedBundle {
    pub fn sign(kp: &KeyPair, payload: &[u8]) -> Self {
        let mut header = Vec::new();
        header.extend_from_slice(MDL_MAGIC);
        header.extend_from_slice(&MDL_FORMAT_VERSION.to_le_bytes());
        header.extend_from_slice(&kp.public_key_bytes());
        header.extend_from_slice(&(payload.len() as u64).to_le_bytes());

        let mut data_to_sign = header.clone();
        data_to_sign.extend_from_slice(payload);

        let signature = kp.signing_key.sign(&data_to_sign);

        Self {
            header,
            signature,
            payload: payload.to_vec(),
        }
    }

    pub fn verify(&self) -> Result<(), String> {
        // header layout: magic(4) + version(2) + pubkey(32) + payload_len(8)
        let pubkey_bytes: [u8; 32] = self.header[6..38]
            .try_into()
            .map_err(|_| "invalid header".to_string())?;
        self.verify_with_key(&pubkey_bytes)
    }

    /// 嵌入 header 的签名者公钥（Engine facade 等外部代码读身份用）
    pub fn signer_pubkey(&self) -> [u8; 32] {
        // header layout: magic(4) + version(2) + pubkey(32) + payload_len(8)
        self.header[6..38].try_into().unwrap_or([0u8; 32])
    }

    pub fn verify_with_key(&self, pubkey: &[u8; 32]) -> Result<(), String> {
        let verifying_key =
            VerifyingKey::from_bytes(pubkey).map_err(|e| format!("invalid key: {}", e))?;

        let mut data_to_verify = self.header.clone();
        data_to_verify.extend_from_slice(&self.payload);

        verifying_key
            .verify(&data_to_verify, &self.signature)
            .map_err(|e| format!("signature invalid: {}", e))
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut Vec<u8> {
        &mut self.payload
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.header);
        out.extend_from_slice(&self.signature.to_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 4 + 2 + 32 + 8 + 64 {
            return Err("too short".into());
        }
        if &data[..4] != MDL_MAGIC {
            return Err("invalid magic".into());
        }

        let header = data[..46].to_vec(); // 4 + 2 + 32 + 8
        let sig_bytes: [u8; 64] = data[46..110]
            .try_into()
            .map_err(|_| "invalid signature length".to_string())?;
        // ed25519-dalek >= 3: `Signature::from_bytes` removed; use `TryFrom<&[u8]>`
        let signature = Signature::try_from(&sig_bytes[..])
            .map_err(|_| "invalid signature".to_string())?;
        let payload = data[110..].to_vec();

        Ok(Self {
            header,
            signature,
            payload,
        })
    }
}