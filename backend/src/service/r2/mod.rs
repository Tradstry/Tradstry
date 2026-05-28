use std::time::Duration;

use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::{Client, Config};

/// Cloudflare R2 is S3-compatible. We talk to it with the AWS S3 SDK pointed at
/// the R2 endpoint, using static credentials and region "auto".
#[derive(Clone)]
pub struct R2Client {
    client: Client,
    bucket: String,
}

impl R2Client {
    pub fn from_env() -> Result<Self> {
        let account_id = std::env::var("R2_ACCOUNT_ID").context("R2_ACCOUNT_ID must be set")?;
        let access_key_id =
            std::env::var("R2_ACCESS_KEY_ID").context("R2_ACCESS_KEY_ID must be set")?;
        let secret_access_key =
            std::env::var("R2_SECRET_ACCESS_KEY").context("R2_SECRET_ACCESS_KEY must be set")?;
        let bucket = std::env::var("R2_BUCKET").context("R2_BUCKET must be set")?;

        let credentials =
            Credentials::new(access_key_id, secret_access_key, None, None, "tradstry-r2");

        let config = Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .endpoint_url(format!("https://{account_id}.r2.cloudflarestorage.com"))
            .credentials_provider(credentials)
            .force_path_style(true)
            .build();

        Ok(Self {
            client: Client::from_conf(config),
            bucket,
        })
    }

    /// Upload an object. `body` is the full file bytes; R2 stores it verbatim.
    /// (Buffered in memory for simplicity — acceptable for the self-hosted,
    /// low-concurrency deployment; revisit with streaming multipart if memory
    /// pressure from large videos becomes an issue.)
    pub async fn put_object(&self, key: &str, body: Vec<u8>, content_type: &str) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(body))
            .content_type(content_type)
            .send()
            .await
            .with_context(|| format!("Failed to upload object to R2: {key}"))?;
        Ok(())
    }

    pub async fn delete_object(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("Failed to delete object from R2: {key}"))?;
        Ok(())
    }

    /// Download an object's raw bytes from R2.
    pub async fn get_object(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        use anyhow::Context;
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("R2 get_object failed for key {key}"))?;
        let data = resp.body.collect().await.context("R2 body read failed")?;
        Ok(data.into_bytes().to_vec())
    }

    /// Generate a time-limited presigned GET URL for an object. This is a local
    /// signing operation (no network round-trip), so it's cheap to call once
    /// per media record on read.
    pub async fn presigned_get_url(&self, key: &str, ttl: Duration) -> Result<String> {
        let presigned = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(PresigningConfig::expires_in(ttl).context("Invalid presigning TTL")?)
            .await
            .with_context(|| format!("Failed to presign R2 object: {key}"))?;
        Ok(presigned.uri().to_string())
    }
}
