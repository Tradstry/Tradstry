//! Per-user token-bucket rate limiting for the MCP transport, backed by Redis so
//! the limit is shared across replicas (and survives process restarts).
//!
//! Agents driving the MCP server can retry aggressively; this caps the request
//! rate per authenticated user to protect the shared backend. The refill-and-
//! consume is a single atomic Lua script keyed on Redis server time (see
//! `RedisClient::token_bucket_check`).

use std::sync::Arc;

use tradstry_backend::service::redis::{RedisClient, TokenBucketResult};

/// Maximum burst (bucket capacity) per user, in tokens (= requests).
const BUCKET_CAPACITY: f64 = 60.0;
/// Sustained refill rate in tokens per second (→ 60 requests/minute steady state).
const REFILL_PER_SEC: f64 = 1.0;

/// Redis-backed per-user token-bucket rate limiter.
pub struct RateLimiter {
    redis: Arc<RedisClient>,
}

impl RateLimiter {
    pub fn new(redis: Arc<RedisClient>) -> Self {
        Self { redis }
    }

    /// Consume one token for `user_id`. Returns `Ok(())` when allowed, or
    /// `Err(retry_after_secs)` when the bucket is empty.
    ///
    /// Fails OPEN: if Redis is unreachable the request is allowed (and logged),
    /// so a Redis outage degrades rate limiting rather than taking the MCP
    /// server down.
    pub async fn check(&self, user_id: &str) -> Result<(), u32> {
        let key = format!("mcp:ratelimit:{user_id}");
        match self
            .redis
            .token_bucket_check(&key, BUCKET_CAPACITY, REFILL_PER_SEC)
            .await
        {
            Ok(TokenBucketResult::Allowed) => Ok(()),
            Ok(TokenBucketResult::Limited { retry_after_secs }) => Err(retry_after_secs),
            Err(e) => {
                tracing::warn!("rate limiter: Redis error, failing open: {e}");
                Ok(())
            }
        }
    }
}
