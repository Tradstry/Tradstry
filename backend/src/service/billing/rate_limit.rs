//! Burst limits on AI endpoints that quotas don't cover.
//!
//! Distinct from `quota`: quotas meter *value* over a billing month and fail
//! closed, these bound *rate* over seconds and **fail open**. A Redis outage
//! must not take autocomplete down — abuse protection is not worth an incident,
//! whereas letting someone exceed a paid limit is.
//!
//! Autocomplete does not consume an AI action; this sits alongside the monthly
//! count rather than inside it. The MCP transport has its own equivalent limiter
//! in `mcp-server/src/rate_limit.rs`, and its tools never reach a model.

use crate::service::redis::client::{RedisClient, TokenBucketResult};

/// Autocomplete fires on a keystroke pause. Twenty in a burst is far more than
/// a person types and far less than a script would.
const AUTOCOMPLETE_BURST: f64 = 20.0;
const AUTOCOMPLETE_PER_SEC: f64 = 20.0 / 60.0;

/// Refused because the caller is going too fast. Distinct from a plan limit:
/// waiting fixes this, upgrading does not.
#[derive(Debug, Clone, Copy)]
pub struct RateLimited {
    pub retry_after_secs: u32,
}

impl std::fmt::Display for RateLimited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Too many requests. Try again in {}s.",
            self.retry_after_secs
        )
    }
}

async fn check(
    redis: Option<&RedisClient>,
    key: String,
    burst: f64,
    per_sec: f64,
) -> Result<(), RateLimited> {
    // No Redis: allow. See the module note on failing open.
    let Some(redis) = redis else {
        return Ok(());
    };

    match redis.token_bucket_check(&key, burst, per_sec).await {
        Ok(TokenBucketResult::Allowed) => Ok(()),
        Ok(TokenBucketResult::Limited { retry_after_secs }) => {
            Err(RateLimited { retry_after_secs })
        }
        Err(e) => {
            log::warn!("[billing] rate-limit check failed for {key}, allowing: {e:#}");
            Ok(())
        }
    }
}

pub async fn check_autocomplete(
    redis: Option<&RedisClient>,
    user_id: &str,
) -> Result<(), RateLimited> {
    check(
        redis,
        format!("billing:rl:autocomplete:{user_id}"),
        AUTOCOMPLETE_BURST,
        AUTOCOMPLETE_PER_SEC,
    )
    .await
}
