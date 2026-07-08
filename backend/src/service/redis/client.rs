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

/// Outcome of a token-bucket rate-limit check.
pub enum TokenBucketResult {
    /// Request allowed; one token was consumed.
    Allowed,
    /// Request denied; retry after this many seconds.
    Limited { retry_after_secs: u32 },
}

/// Atomic token-bucket refill-and-consume, evaluated server-side in a single Lua
/// script so it is race-free across replicas. Uses Redis `TIME` as the clock
/// (one authoritative source, no cross-node skew). Returns `{allowed, retry}`.
const TOKEN_BUCKET_LUA: &str = r#"
local capacity = tonumber(ARGV[1])
local refill = tonumber(ARGV[2])
local t = redis.call('TIME')
local now = tonumber(t[1]) + tonumber(t[2]) / 1000000.0
local d = redis.call('HMGET', KEYS[1], 'tokens', 'ts')
local tokens = tonumber(d[1])
local ts = tonumber(d[2])
if tokens == nil then tokens = capacity; ts = now end
local elapsed = now - ts
if elapsed < 0 then elapsed = 0 end
tokens = math.min(capacity, tokens + elapsed * refill)
local allowed = 0
local retry = 0
if tokens >= 1 then
  tokens = tokens - 1
  allowed = 1
else
  retry = math.ceil((1 - tokens) / refill)
end
redis.call('HSET', KEYS[1], 'tokens', tokens, 'ts', now)
redis.call('PEXPIRE', KEYS[1], math.ceil(capacity / refill * 1000) + 1000)
return {allowed, retry}
"#;

impl RedisClient {
    /// Atomic token-bucket rate-limit check for `key`. Consumes one token when
    /// allowed. Refill and consume happen in one Lua script keyed on Redis server
    /// time, so it is correct across multiple MCP replicas sharing this Redis.
    pub async fn token_bucket_check(
        &self,
        key: &str,
        capacity: f64,
        refill_per_sec: f64,
    ) -> Result<TokenBucketResult> {
        let mut conn = self.conn.clone();
        let res: Vec<i64> = redis::Script::new(TOKEN_BUCKET_LUA)
            .key(key)
            .arg(capacity)
            .arg(refill_per_sec)
            .invoke_async(&mut conn)
            .await
            .context("token bucket script failed")?;

        if res.first().copied().unwrap_or(0) == 1 {
            Ok(TokenBucketResult::Allowed)
        } else {
            let retry_after_secs = res.get(1).copied().unwrap_or(1).max(1) as u32;
            Ok(TokenBucketResult::Limited { retry_after_secs })
        }
    }
}
