use anyhow::{Context, Result};
use log::{error, info};
use redis::AsyncCommands;

#[derive(Clone)]
pub struct RedisClient {
    conn: redis::aio::ConnectionManager,
}

impl RedisClient {
    pub async fn from_env() -> Result<Self> {
        let url = std::env::var("REDIS_URL").context("REDIS_URL environment variable not set")?;
        let client = redis::Client::open(url.as_str()).context("Failed to create Redis client")?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .context("Failed to connect to Redis")?;
        info!("Redis connection established");
        Ok(Self { conn })
    }

    /// GET a key, returning None on miss or error.
    pub async fn get(&self, key: &str) -> Option<String> {
        let mut conn = self.conn.clone();
        match conn.get::<_, Option<String>>(key).await {
            Ok(val) => val,
            Err(e) => {
                error!("[redis] GET {key} failed: {e}");
                None
            }
        }
    }

    /// SET a key with an expiry in seconds. Logs and swallows errors.
    pub async fn set_ex(&self, key: &str, value: &str, ttl_secs: u64) {
        let mut conn = self.conn.clone();
        if let Err(e) = conn.set_ex::<_, _, ()>(key, value, ttl_secs).await {
            error!("[redis] SET {key} failed: {e}");
        }
    }

    /// Delete all keys matching a pattern using KEYS + DEL.
    /// Logs and swallows errors so callers are never blocked.
    pub async fn delete_by_prefix(&self, pattern: &str) {
        let mut conn = self.conn.clone();
        let keys: Vec<String> = match redis::cmd("KEYS").arg(pattern).query_async(&mut conn).await {
            Ok(k) => k,
            Err(e) => {
                error!("[redis] KEYS {pattern} failed: {e}");
                return;
            }
        };
        if keys.is_empty() {
            return;
        }
        if let Err(e) = conn.del::<_, ()>(&keys).await {
            error!("[redis] DEL {} keys failed: {e}", keys.len());
        }
    }
}
