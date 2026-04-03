use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng},
};
use aes_gcm::aead::rand_core::RngCore;
use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

fn encryption_key() -> Result<[u8; 32]> {
    let key_str =
        std::env::var("BROKERAGE_ENCRYPTION_KEY").context("BROKERAGE_ENCRYPTION_KEY not set")?;
    let decoded = BASE64
        .decode(&key_str)
        .context("BROKERAGE_ENCRYPTION_KEY must be valid base64")?;
    if decoded.len() != 32 {
        return Err(anyhow!(
            "BROKERAGE_ENCRYPTION_KEY must decode to exactly 32 bytes, got {}",
            decoded.len()
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

pub fn encrypt_secret(plaintext: &str) -> Result<String> {
    let key = encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).context("Failed to create cipher")?;

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {e}"))?;

    // Format: base64(nonce || ciphertext)
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

pub fn decrypt_secret(encoded: &str) -> Result<String> {
    let key = encryption_key()?;
    let cipher = Aes256Gcm::new_from_slice(&key).context("Failed to create cipher")?;

    let combined = BASE64
        .decode(encoded)
        .context("Invalid base64 in encrypted secret")?;
    if combined.len() < 12 {
        return Err(anyhow!("Encrypted secret too short"));
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption failed: {e}"))?;

    String::from_utf8(plaintext).context("Decrypted secret is not valid UTF-8")
}
