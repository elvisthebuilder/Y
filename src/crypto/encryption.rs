use anyhow::{anyhow, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use rand::rngs::OsRng;
use rand::RngCore;
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};

pub struct EncryptedPayload {
    pub ephemeral_public: [u8; 32],
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

pub fn encrypt_for_recipient(
    plaintext: &[u8],
    recipient_public: &PublicKey,
) -> Result<EncryptedPayload> {
    let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    let shared_secret = ephemeral_secret.diffie_hellman(recipient_public);
    let key = derive_key(&shared_secret);

    let cipher =
        ChaCha20Poly1305::new_from_slice(&key).map_err(|e| anyhow!("cipher init: {}", e))?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow!("encryption failed: {}", e))?;

    Ok(EncryptedPayload {
        ephemeral_public: ephemeral_public.to_bytes(),
        nonce: nonce_bytes,
        ciphertext,
    })
}

pub fn decrypt_payload(
    payload: &EncryptedPayload,
    recipient_secret: &x25519_dalek::StaticSecret,
) -> Result<Vec<u8>> {
    let ephemeral_public = PublicKey::from(payload.ephemeral_public);
    let shared_secret = recipient_secret.diffie_hellman(&ephemeral_public);
    let key = derive_key(&shared_secret);

    let cipher =
        ChaCha20Poly1305::new_from_slice(&key).map_err(|e| anyhow!("cipher init: {}", e))?;

    let nonce = Nonce::from_slice(&payload.nonce);

    cipher
        .decrypt(nonce, payload.ciphertext.as_ref())
        .map_err(|e| anyhow!("decryption failed: {}", e))
}

fn derive_key(shared: &SharedSecret) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(shared.as_bytes());
    hasher.finalize().into()
}
