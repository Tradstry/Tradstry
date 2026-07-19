//! Plan-limit guards.
//!
//! Two rules make this safe:
//!
//! 1. **Reserve before spending.** The AI counter is incremented by a single
//!    atomic SQL statement *before* the Gemini call. The cached entitlements
//!    supply the limit and the window, never the enforcement decision — so a
//!    stale cache can't let anyone over-consume.
//! 2. **Fail closed.** If entitlements can't be resolved, revenue-bearing
//!    actions are refused rather than allowed.

use std::fmt;

use async_graphql::ErrorExtensions;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::entitlements::{self, AI_ACTIONS, Entitlements};
use crate::service::db::schema::tables::billing_table;
use crate::service::redis::client::RedisClient;

/// Stable error code the frontend keys off to render an upgrade prompt.
pub const PLAN_LIMIT_CODE: &str = "PLAN_LIMIT_REACHED";

#[derive(Debug, Clone)]
pub enum QuotaError {
    Exhausted {
        resource: &'static str,
        limit: i64,
        resets_at: Option<DateTime<Utc>>,
    },
    /// Entitlements could not be resolved — deny rather than grant.
    Unavailable,
}

impl fmt::Display for QuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuotaError::Exhausted {
                resource,
                limit,
                resets_at,
            } => match resets_at {
                Some(at) => write!(
                    f,
                    "You've reached your {resource} limit ({limit}) for this period. Resets {}.",
                    at.format("%-d %b")
                ),
                None => write!(f, "You've reached your {resource} limit ({limit})."),
            },
            QuotaError::Unavailable => {
                write!(f, "Could not verify your plan. Please try again.")
            }
        }
    }
}

impl std::error::Error for QuotaError {}

impl QuotaError {
    /// The same shape the GraphQL error extensions carry, for the REST upload
    /// path — one client-side helper then recognises a plan limit from either
    /// surface.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            QuotaError::Exhausted {
                resource,
                limit,
                resets_at,
            } => serde_json::json!({
                "code": PLAN_LIMIT_CODE,
                "resource": resource,
                "limit": limit,
                "resetsAt": resets_at.map(|at| at.to_rfc3339()),
                "message": self.to_string(),
            }),
            QuotaError::Unavailable => serde_json::json!({
                "code": "PLAN_UNAVAILABLE",
                "message": self.to_string(),
            }),
        }
    }
}

/// Resolvers surface these with `.map_err(|e| e.extend())?` so the frontend gets
/// a stable `code` plus the limit and reset date to render an upgrade prompt.
///
/// Implemented as `ErrorExtensions` rather than `From`, because async-graphql
/// already blanket-converts any `Display` error.
impl ErrorExtensions for QuotaError {
    fn extend(&self) -> async_graphql::Error {
        async_graphql::Error::new(self.to_string()).extend_with(|_, ext| match self {
            QuotaError::Exhausted {
                resource,
                limit,
                resets_at,
            } => {
                ext.set("code", PLAN_LIMIT_CODE);
                ext.set("resource", *resource);
                ext.set("limit", *limit);
                if let Some(at) = resets_at {
                    ext.set("resetsAt", at.to_rfc3339());
                }
            }
            QuotaError::Unavailable => {
                ext.set("code", "PLAN_UNAVAILABLE");
            }
        })
    }
}

async fn entitlements_or_deny(
    pool: &PgPool,
    redis: Option<&RedisClient>,
    user_id: &str,
) -> Result<Entitlements, QuotaError> {
    entitlements::resolve(pool, redis, user_id)
        .await
        .map_err(|e| {
            log::error!("[billing] failed to resolve entitlements for {user_id}: {e}");
            QuotaError::Unavailable
        })
}

/// Reserve one AI action. Call this **before** the LLM request; on `Ok` the
/// action has already been counted.
///
/// Unlimited plans still increment (with an effectively infinite ceiling) so
/// usage stays visible in the UI.
pub async fn reserve_ai_action(
    pool: &PgPool,
    redis: Option<&RedisClient>,
    user_id: &str,
) -> Result<(), QuotaError> {
    let ent = entitlements_or_deny(pool, redis, user_id).await?;
    let limit = ent.ai.limit.unwrap_or(i32::MAX as i64);

    let reserved = billing_table::reserve_counter(
        pool,
        user_id,
        AI_ACTIONS,
        ent.period_start,
        ent.period_end,
        limit.min(i32::MAX as i64) as i32,
    )
    .await
    .map_err(|e| {
        log::error!("[billing] ai reservation failed for {user_id}: {e}");
        QuotaError::Unavailable
    })?;

    match reserved {
        Some(_) => {
            // Keep the displayed usage honest; enforcement never relied on it.
            entitlements::invalidate(redis, user_id).await;
            Ok(())
        }
        None => Err(QuotaError::Exhausted {
            resource: "AI actions",
            limit,
            resets_at: Some(ent.period_end),
        }),
    }
}

/// Refuse a new brokerage connection when the ceiling is reached. Existing
/// connections are never touched — a downgrade must not break them.
pub async fn check_connection_headroom(
    pool: &PgPool,
    redis: Option<&RedisClient>,
    user_id: &str,
) -> Result<(), QuotaError> {
    let ent = entitlements_or_deny(pool, redis, user_id).await?;

    if ent.connections.is_exhausted() {
        return Err(QuotaError::Exhausted {
            resource: "brokerage connections",
            limit: ent.connections.limit.unwrap_or_default(),
            resets_at: None,
        });
    }
    Ok(())
}

/// Refuse an upload that would cross the media cap, before it reaches R2.
pub async fn check_media_headroom(
    pool: &PgPool,
    redis: Option<&RedisClient>,
    user_id: &str,
    bytes: i64,
) -> Result<(), QuotaError> {
    let ent = entitlements_or_deny(pool, redis, user_id).await?;

    if ent.media.would_exceed(bytes) {
        return Err(QuotaError::Exhausted {
            resource: "media storage",
            limit: ent.media.limit.unwrap_or_default(),
            resets_at: None,
        });
    }
    Ok(())
}

/// Gate the things that *grow* stored data (brokerage sync). Never gate the
/// manual journal/note write path — losing a trade mid-session is worse than
/// briefly exceeding a soft cap.
pub async fn check_data_headroom(
    pool: &PgPool,
    redis: Option<&RedisClient>,
    user_id: &str,
) -> Result<(), QuotaError> {
    let ent = entitlements_or_deny(pool, redis, user_id).await?;

    if ent.data.is_exhausted() {
        return Err(QuotaError::Exhausted {
            resource: "data storage",
            limit: ent.data.limit.unwrap_or_default(),
            resets_at: None,
        });
    }
    Ok(())
}
