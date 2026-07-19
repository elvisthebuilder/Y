use anyhow::Result;
use base64::Engine;
use curve25519_dalek::edwards::CompressedEdwardsY;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

#[derive(Clone)]
pub struct Identity {
    signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicIdentity {
    pub address: String,
    pub verifying_key_bytes: [u8; 32],
    pub alias: String,
}

impl PublicIdentity {
    pub fn handle(&self) -> String {
        super::alias::display_handle(&self.alias, &self.address)
    }

    pub fn short_id(&self) -> String {
        super::alias::short_address(&self.address)
    }
}

impl Identity {
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let address = Self::derive_address(&verifying_key);

        Self {
            signing_key,
            verifying_key,
            address,
        }
    }

    pub fn from_bytes(secret: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(secret);
        let verifying_key = signing_key.verifying_key();
        let address = Self::derive_address(&verifying_key);

        Self {
            signing_key,
            verifying_key,
            address,
        }
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.signing_key.sign(message)
    }

    pub fn verify(verifying_key: &VerifyingKey, message: &[u8], signature: &Signature) -> bool {
        verifying_key.verify(message, signature).is_ok()
    }

    pub fn public_identity(&self) -> PublicIdentity {
        PublicIdentity {
            address: self.address.clone(),
            verifying_key_bytes: self.verifying_key.to_bytes(),
            alias: super::alias::generate_alias(),
        }
    }

    pub fn secret_bytes(&self) -> &[u8; 32] {
        self.signing_key.as_bytes()
    }

    fn derive_address(key: &VerifyingKey) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let hash = hasher.finalize();
        format!(
            "root:{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&hash[..16])
        )
    }
}

impl PublicIdentity {
    pub fn verifying_key(&self) -> Result<VerifyingKey> {
        Ok(VerifyingKey::from_bytes(&self.verifying_key_bytes)?)
    }

    pub fn matches_search(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.alias.to_lowercase().contains(&q)
            || self.address.to_lowercase().contains(&q)
            || self.handle().to_lowercase().contains(&q)
    }
}

impl Identity {
    pub fn public_identity_with_alias(&self, alias: String) -> PublicIdentity {
        PublicIdentity {
            address: self.address.clone(),
            verifying_key_bytes: self.verifying_key.to_bytes(),
            alias,
        }
    }

    pub fn x25519_secret(&self) -> X25519StaticSecret {
        let mut hasher = Sha512::new();
        hasher.update(self.signing_key.as_bytes());
        let hash = hasher.finalize();
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&hash[..32]);
        // Clamp per RFC 7748
        key_bytes[0] &= 248;
        key_bytes[31] &= 127;
        key_bytes[31] |= 64;
        X25519StaticSecret::from(key_bytes)
    }

    pub fn x25519_public(&self) -> X25519PublicKey {
        X25519PublicKey::from(&self.x25519_secret())
    }
}

pub fn verifying_key_to_x25519(vk: &VerifyingKey) -> Option<X25519PublicKey> {
    let compressed = CompressedEdwardsY(vk.to_bytes());
    let edwards = compressed.decompress()?;
    let montgomery = edwards.to_montgomery();
    Some(X25519PublicKey::from(montgomery.to_bytes()))
}
