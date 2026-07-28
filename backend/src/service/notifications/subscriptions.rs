use anyhow::{Context, Result};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PushSubscription {
    pub id: String,
    pub user_id: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

/// Keyed on endpoint, so a browser that re-subscribes refreshes its keys in place
/// rather than leaving a stale row that will only ever 410.
pub async fn upsert(
    pool: &PgPool,
    user_id: &str,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    user_agent: Option<&str>,
) -> Result<String> {
    let row = sqlx::query(
        "INSERT INTO push_subscriptions (id, user_id, endpoint, p256dh, auth, user_agent) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (endpoint) DO UPDATE \
           SET user_id = EXCLUDED.user_id, \
               p256dh = EXCLUDED.p256dh, \
               auth = EXCLUDED.auth, \
               user_agent = EXCLUDED.user_agent \
         RETURNING id",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(endpoint)
    .bind(p256dh)
    .bind(auth)
    .bind(user_agent)
    .fetch_one(pool)
    .await
    .context("failed to upsert push subscription")?;
    Ok(row.try_get("id")?)
}

pub async fn delete_by_endpoint(pool: &PgPool, user_id: &str, endpoint: &str) -> Result<bool> {
    let deleted =
        sqlx::query("DELETE FROM push_subscriptions WHERE user_id = $1 AND endpoint = $2")
            .bind(user_id)
            .bind(endpoint)
            .execute(pool)
            .await
            .context("failed to delete push subscription")?;
    Ok(deleted.rows_affected() == 1)
}

pub async fn delete_by_id(pool: &PgPool, id: &str) -> Result<()> {
    sqlx::query("DELETE FROM push_subscriptions WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .context("failed to delete push subscription")?;
    Ok(())
}

pub async fn list_for_user(pool: &PgPool, user_id: &str) -> Result<Vec<PushSubscription>> {
    let rows = sqlx::query(
        "SELECT id, user_id, endpoint, p256dh, auth FROM push_subscriptions \
         WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("failed to list push subscriptions")?;

    rows.into_iter()
        .map(|r| {
            Ok(PushSubscription {
                id: r.try_get("id")?,
                user_id: r.try_get("user_id")?,
                endpoint: r.try_get("endpoint")?,
                p256dh: r.try_get("p256dh")?,
                auth: r.try_get("auth")?,
            })
        })
        .collect()
}

pub async fn touch_success(pool: &PgPool, id: &str) -> Result<()> {
    sqlx::query("UPDATE push_subscriptions SET last_success_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .context("failed to record push success")?;
    Ok(())
}
