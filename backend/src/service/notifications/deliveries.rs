use anyhow::{Context, Result};
use sqlx::{PgConnection, PgPool, Row};
use std::time::Duration;

const BACKOFF_BASE_SECS: u64 = 30;
const BACKOFF_CAP_SECS: u64 = 6 * 60 * 60;

#[derive(Debug, Clone)]
pub struct DueDelivery {
    pub notification_id: String,
    pub subscription_id: String,
    pub attempts: i32,
    pub title: String,
    pub body: String,
    pub deep_link: Option<String>,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

/// Doubling from 30s, capped at 6h. Past the cap the endpoint is not coming back,
/// which is what the attempt ceiling in `mark_retry` is for.
pub fn backoff(attempts: i32) -> Duration {
    let exponent = attempts.max(1) as u32 - 1;
    let secs = BACKOFF_BASE_SECS
        .checked_shl(exponent)
        .unwrap_or(BACKOFF_CAP_SECS)
        .min(BACKOFF_CAP_SECS);
    Duration::from_secs(secs)
}

/// One row per browser the user has registered *now*. A browser that subscribes
/// later gets no row, so it receives no push for news it was never present for.
///
/// `send_after` defers the push out of quiet hours. The feed row is untouched —
/// suppressing creation would lose the record, dropping the push would lose the
/// nudge, so only its timing moves.
pub async fn fan_out(
    conn: &mut PgConnection,
    notification_id: &str,
    user_id: &str,
    send_after: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<u64> {
    let inserted = sqlx::query(
        "INSERT INTO notification_deliveries (notification_id, subscription_id, next_attempt_at) \
         SELECT $1, s.id, COALESCE($3, now()) FROM push_subscriptions s WHERE s.user_id = $2 \
         ON CONFLICT (notification_id, subscription_id) DO NOTHING",
    )
    .bind(notification_id)
    .bind(user_id)
    .bind(send_after)
    .execute(conn)
    .await
    .context("failed to fan out notification deliveries")?;
    Ok(inserted.rows_affected())
}

pub async fn claim_due(conn: &mut PgConnection, limit: i64) -> Result<Vec<DueDelivery>> {
    let rows = sqlx::query(
        "SELECT d.notification_id, d.subscription_id, d.attempts, \
                n.title, n.body, n.deep_link, \
                s.endpoint, s.p256dh, s.auth \
         FROM notification_deliveries d \
         JOIN notifications n ON n.id = d.notification_id \
         JOIN push_subscriptions s ON s.id = d.subscription_id \
         WHERE d.status = 'pending' AND d.next_attempt_at <= now() \
         ORDER BY d.next_attempt_at \
         LIMIT $1 \
         FOR UPDATE OF d SKIP LOCKED",
    )
    .bind(limit)
    .fetch_all(conn)
    .await
    .context("failed to claim due deliveries")?;

    rows.into_iter()
        .map(|r| {
            Ok(DueDelivery {
                notification_id: r.try_get("notification_id")?,
                subscription_id: r.try_get("subscription_id")?,
                attempts: r.try_get("attempts")?,
                title: r.try_get("title")?,
                body: r.try_get("body")?,
                deep_link: r.try_get("deep_link")?,
                endpoint: r.try_get("endpoint")?,
                p256dh: r.try_get("p256dh")?,
                auth: r.try_get("auth")?,
            })
        })
        .collect()
}

pub async fn mark_sent(pool: &PgPool, notification_id: &str, subscription_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE notification_deliveries SET status = 'sent', sent_at = now(), last_error = NULL \
         WHERE notification_id = $1 AND subscription_id = $2",
    )
    .bind(notification_id)
    .bind(subscription_id)
    .execute(pool)
    .await
    .context("failed to mark delivery sent")?;
    Ok(())
}

pub async fn mark_gone(pool: &PgPool, notification_id: &str, subscription_id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE notification_deliveries SET status = 'gone' \
         WHERE notification_id = $1 AND subscription_id = $2",
    )
    .bind(notification_id)
    .bind(subscription_id)
    .execute(pool)
    .await
    .context("failed to mark delivery gone")?;
    Ok(())
}

pub async fn mark_retry(
    pool: &PgPool,
    notification_id: &str,
    subscription_id: &str,
    attempts: i32,
    error: &str,
    max_attempts: i32,
) -> Result<()> {
    let next = backoff(attempts);
    sqlx::query(
        "UPDATE notification_deliveries \
         SET attempts = $3, \
             last_error = $4, \
             status = CASE WHEN $3 >= $5 THEN 'failed' ELSE 'pending' END, \
             next_attempt_at = now() + make_interval(secs => $6) \
         WHERE notification_id = $1 AND subscription_id = $2",
    )
    .bind(notification_id)
    .bind(subscription_id)
    .bind(attempts)
    .bind(error)
    .bind(max_attempts)
    .bind(next.as_secs() as f64)
    .execute(pool)
    .await
    .context("failed to schedule delivery retry")?;
    Ok(())
}
