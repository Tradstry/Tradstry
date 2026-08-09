use anyhow::{Context, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::future::Future;

use crate::service::db::client::UserDb;
use crate::service::redis::client::RedisClient;

const CACHE_TTL_SECS: u64 = 300;

async fn workspace_version(user_db: &UserDb, workspace_id: &str) -> Result<String> {
    sqlx::query_scalar(
        "SELECT concat(
            COALESCE((SELECT extract(epoch FROM max(updated_at))::text
                      FROM journal_entries
                      WHERE user_id = $1 AND workspace_id = $2), '0'),
            ':',
            COALESCE((SELECT extract(epoch FROM updated_at)::text
                      FROM workspaces WHERE id = $2 AND user_id = $1), '0')
         )",
    )
    .bind(user_db.user_id())
    .bind(workspace_id)
    .fetch_one(user_db.pool())
    .await
    .context("Failed to read analytics cache version")
}

/// Versioned cache: journal or workspace writes change the key immediately, so
/// the TTL is only cleanup and never a stale-data window.
pub async fn get_or_load<T, F, Fut>(
    redis: &RedisClient,
    user_db: &UserDb,
    workspace_id: &str,
    kind: &str,
    parameters: &str,
    load: F,
) -> Result<T>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let version = workspace_version(user_db, workspace_id).await?;
    let parameter_hash = format!("{:x}", md5::compute(parameters.as_bytes()));
    let key = format!(
        "analytics:{kind}:{}:{workspace_id}:{version}:{parameter_hash}",
        user_db.user_id()
    );

    if let Some(cached) = redis.get(&key).await
        && let Ok(value) = serde_json::from_str(&cached)
    {
        return Ok(value);
    }

    let value = load().await?;
    if let Ok(serialized) = serde_json::to_string(&value) {
        redis.set_ex(&key, &serialized, CACHE_TTL_SECS).await;
    }
    Ok(value)
}
