//! Plan-limit guards for resolvers.
//!
//! Thin `Context` adapters over `service::billing::quota` so a chokepoint is a
//! single line at the top of a resolver. Errors carry the `PLAN_LIMIT_REACHED`
//! extension the frontend keys off.

use std::sync::Arc;

use async_graphql::{Context, ErrorExtensions, Result};
use clerk_rs::validators::authorizer::ClerkJwt;

use crate::service::billing::quota;
use crate::service::db::Db;
use crate::service::read_service::users::ensure_user;
use crate::service::redis::client::RedisClient;

fn redis(ctx: &Context<'_>) -> Option<Arc<RedisClient>> {
    ctx.data::<Arc<RedisClient>>().ok().cloned()
}

/// The caller's internal user id. Resolvers that already hold one should pass
/// it to the guards directly rather than paying for this again.
pub async fn current_user(ctx: &Context<'_>) -> Result<(Arc<Db>, String)> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let db = ctx.data::<Arc<Db>>()?;
    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let user = ensure_user(db.pool(), &jwt.sub, full_name, email).await?;
    Ok((db.clone(), user.id))
}

/// Reserve one AI action. Call this **before** the model request — on `Ok` the
/// action is already counted.
pub async fn reserve_ai(ctx: &Context<'_>, user_id: &str) -> Result<()> {
    let db = ctx.data::<Arc<Db>>()?;
    quota::reserve_ai_action(db.pool(), redis(ctx).as_deref(), user_id)
        .await
        .map_err(|e| e.extend())
}

/// Burst limit, not a quota — the error tells the caller to wait, not upgrade.
pub async fn check_autocomplete_rate(ctx: &Context<'_>, user_id: &str) -> Result<()> {
    crate::service::billing::rate_limit::check_autocomplete(redis(ctx).as_deref(), user_id)
        .await
        .map_err(|e| {
            async_graphql::Error::new(e.to_string()).extend_with(|_, ext| {
                ext.set("code", "RATE_LIMITED");
                ext.set("retryAfterSecs", e.retry_after_secs);
            })
        })
}

pub async fn require_connection_headroom(ctx: &Context<'_>, user_id: &str) -> Result<()> {
    let db = ctx.data::<Arc<Db>>()?;
    quota::check_connection_headroom(db.pool(), redis(ctx).as_deref(), user_id)
        .await
        .map_err(|e| e.extend())
}
