use anyhow::{Context, Result};
use sqlx::PgPool;

use super::ALL_EVENT_TYPES;

#[derive(Debug, Clone)]
pub struct PreferenceRow {
    pub event_type: String,
    pub enabled: bool,
}

/// A missing row means enabled. The default lives in the query, not in the
/// callers, so no new call site can forget it and quietly invert the semantics.
pub async fn is_enabled(pool: &PgPool, user_id: &str, event_type: &str) -> Result<bool> {
    let row: (bool,) = sqlx::query_as(
        "SELECT COALESCE(p.enabled, true) \
         FROM (SELECT $2::text AS event_type) e \
         LEFT JOIN notification_preferences p \
           ON p.user_id = $1 AND p.event_type = e.event_type",
    )
    .bind(user_id)
    .bind(event_type)
    .fetch_one(pool)
    .await
    .context("failed to read notification preference")?;
    Ok(row.0)
}

pub async fn set(pool: &PgPool, user_id: &str, event_type: &str, enabled: bool) -> Result<()> {
    sqlx::query(
        "INSERT INTO notification_preferences (user_id, event_type, enabled) \
         VALUES ($1, $2, $3) \
         ON CONFLICT (user_id, event_type) DO UPDATE SET enabled = EXCLUDED.enabled",
    )
    .bind(user_id)
    .bind(event_type)
    .bind(enabled)
    .execute(pool)
    .await
    .context("failed to write notification preference")?;
    Ok(())
}

/// Every known type with its effective state, so the settings UI renders without
/// having to know the catalogue.
pub async fn list(pool: &PgPool, user_id: &str) -> Result<Vec<PreferenceRow>> {
    let mut out = Vec::with_capacity(ALL_EVENT_TYPES.len());
    for event_type in ALL_EVENT_TYPES {
        out.push(PreferenceRow {
            event_type: event_type.to_string(),
            enabled: is_enabled(pool, user_id, event_type).await?,
        });
    }
    Ok(out)
}
