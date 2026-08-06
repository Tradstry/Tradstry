use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng},
};
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

/// Registers a SnapTrade user and stores the resulting secret as one unit.
///
/// Registration is destructive on SnapTrade's side — an existing user is deleted
/// and recreated, invalidating the old secret. If we then failed to persist the
/// new one, the account would be permanently unable to authenticate with no way
/// to recover the secret. There is no distributed transaction available here, so
/// a failed write is compensated by deleting the user we just created, leaving
/// the account cleanly unregistered and re-connectable.
pub async fn register_and_store(
    client: &crate::service::brokerage::client::BrokerageClient,
    pool: &sqlx::PgPool,
    workspace_id: &str,
    user_id: &str,
) -> Result<crate::service::brokerage::client::CreateUserResponse> {
    let reg = client.register_user(user_id).await?;

    let encrypted = encrypt_secret(&reg.user_secret)?;
    let stored =
        crate::service::db::schema::tables::workspaces_table::update_snaptrade_credentials(
            pool,
            workspace_id,
            user_id,
            &reg.user_id,
            &encrypted,
            None,
        )
        .await;

    if let Err(e) = stored {
        log::error!(
            "Failed to persist SnapTrade secret for account={workspace_id} after registration \
             ({e}) — rolling back the SnapTrade user to keep both sides consistent"
        );
        if let Err(cleanup) = client.delete_user(&reg.user_id).await {
            log::error!(
                "Rollback failed for SnapTrade user {}: {cleanup}. The account now holds no \
                 usable secret and must be reconnected.",
                reg.user_id
            );
        }
        return Err(e).context("Failed to store SnapTrade credentials");
    }

    Ok(reg)
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
